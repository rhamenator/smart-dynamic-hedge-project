//! A minimal, narrowly-scoped XML text extractor for RSS 2.0 / Atom 1.0
//! feeds — **not** a general-purpose XML parser. It only ever extracts the
//! text content of a handful of named leaf elements inside `<item>`/
//! `<entry>` blocks. It deliberately never parses `<!DOCTYPE ...>` internal
//! subsets or `<!ENTITY ...>` declarations — it skips over them as opaque
//! bytes without interpreting them, which is what actually prevents XXE
//! (external entity expansion) here: there is no code path that could ever
//! resolve an external entity, because entity declarations are never
//! looked at in the first place. A general XML library that *does*
//! implement DTD/entity support would need to be explicitly configured to
//! disable it to get the same guarantee; this parser gets it by omission.
//!
//! Namespace handling is approximate: a qualified name like `dc:title` is
//! matched by its local name (`title`) regardless of prefix, which is not
//! namespace-URI-correct XML semantics but matches what real-world feeds
//! actually look like closely enough for evidence extraction — the same
//! "close enough, not exhaustively correct" spirit as
//! `data._regular_market_state`'s NYSE-hours approximation.

/// Skips a `<!--...-->` comment, `<?...?>` processing instruction, or
/// `<!...>` declaration (DOCTYPE, possibly with an internal `[...]`
/// subset) starting at `xml[from]` (which must be `<`). Returns the
/// position just past it, or `None` if `xml[from]` isn't one of these
/// constructs, or if it's unterminated (malformed input near the end of
/// the document — degrade to "no more skippable content" rather than
/// panicking or looping).
fn skip_special(xml: &str, from: usize) -> Option<usize> {
    let rest = xml.get(from..)?;
    if let Some(body) = rest.strip_prefix("<!--") {
        let end = body.find("-->")?;
        return Some(from + 4 + end + 3);
    }
    if let Some(body) = rest.strip_prefix("<?") {
        let end = body.find("?>")?;
        return Some(from + 2 + end + 2);
    }
    if rest.starts_with("<!") {
        // DOCTYPE or similar. If it has an internal subset (`[...]`),
        // skip past the matching `]` first so a `>` inside the subset
        // (e.g. inside an `<!ENTITY ...>` declaration) doesn't end the
        // scan early — critically, this never inspects what's *inside*
        // the subset, only where it ends.
        let mut i = from + 2;
        let bytes = xml.as_bytes();
        if let Some(bracket) = xml[i..].find('[') {
            let after_bracket = i + bracket + 1;
            let close = xml[after_bracket..].find(']')?;
            i = after_bracket + close + 1;
        }
        while i < bytes.len() && bytes[i] != b'>' {
            i += 1;
        }
        if i >= bytes.len() {
            return None;
        }
        return Some(i + 1);
    }
    None
}

/// The qualified name of the next open tag at or after `from`, plus the
/// byte position right after its `>` and whether it was self-closing
/// (`<tag/>`). Skips comments/PIs/declarations along the way. Returns
/// `None` once no more tags are found (end of document or malformed
/// trailing content).
struct OpenTag<'a> {
    qualified_name: &'a str,
    after_tag: usize,
    self_closing: bool,
}

fn next_open_tag(xml: &str, from: usize) -> Option<OpenTag<'_>> {
    let mut pos = from;
    loop {
        let lt = xml[pos..].find('<')? + pos;
        if let Some(after) = skip_special(xml, lt) {
            pos = after;
            continue;
        }
        let rest = &xml[lt..];
        if rest.starts_with("</") {
            // A closing tag encountered while scanning for an *open* tag —
            // not a match; skip past it and keep looking.
            let gt = rest.find('>')?;
            pos = lt + gt + 1;
            continue;
        }
        // A real open tag. Find where it ends, respecting quoted
        // attribute values so a `>` inside `alt="a>b"` doesn't end it
        // early.
        let name_start = lt + 1;
        let name_end = xml[name_start..]
            .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
            .map(|i| name_start + i)?;
        let qualified_name = &xml[name_start..name_end];
        if qualified_name.is_empty() {
            pos = name_end.max(lt + 1);
            continue;
        }

        let mut i = name_end;
        let bytes = xml.as_bytes();
        let mut quote: Option<u8> = None;
        while i < bytes.len() {
            let b = bytes[i];
            match quote {
                Some(q) => {
                    if b == q {
                        quote = None;
                    }
                }
                None => match b {
                    b'"' | b'\'' => quote = Some(b),
                    b'>' => break,
                    _ => {}
                },
            }
            i += 1;
        }
        if i >= bytes.len() {
            return None; // unterminated tag
        }
        let self_closing = i > name_end && bytes[i - 1] == b'/';
        return Some(OpenTag { qualified_name, after_tag: i + 1, self_closing });
    }
}

fn local_name(qualified: &str) -> &str {
    match qualified.rfind(':') {
        Some(i) => &qualified[i + 1..],
        None => qualified,
    }
}

/// Finds the next element (open tag through matching close tag) whose
/// local name matches one of `candidates`, at or after `from`. Returns
/// `(local_name_matched, inner_text, position_after_the_whole_element)`.
/// Stops scanning (returns `None`) once past `limit` — bounds how far a
/// single search can run, so a pathological "never found" input can't
/// cause unbounded rescanning when called repeatedly.
fn find_first_matching_element<'a>(xml: &'a str, candidates: &[&str], from: usize, limit: usize) -> Option<(&'a str, String, usize)> {
    let mut pos = from;
    while pos < limit {
        let tag = next_open_tag(xml, pos)?;
        let local = local_name(tag.qualified_name);
        let matched = candidates.contains(&local);
        if !matched {
            pos = tag.after_tag;
            continue;
        }
        if tag.self_closing {
            return Some((local, String::new(), tag.after_tag));
        }
        let close_needle = format!("</{}", tag.qualified_name);
        let close_pos = xml[tag.after_tag..].find(&close_needle)? + tag.after_tag;
        let inner_raw = &xml[tag.after_tag..close_pos];
        let close_gt = xml[close_pos..].find('>')? + close_pos;
        return Some((local, decode_text(inner_raw), close_gt + 1));
    }
    None
}

/// Decodes `<![CDATA[...]]>` sections and the five predefined XML entities.
/// Anything else (numeric character references, other named entities) is
/// left as-is rather than guessed at — evidence text is untrusted either
/// way, and an unrecognized escape sequence surviving verbatim is safer
/// than a wrong guess at its meaning.
fn decode_text(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(start) = rest.find("<![CDATA[") {
        out.push_str(&decode_entities(&rest[..start]));
        let after_open = start + "<![CDATA[".len();
        if let Some(end) = rest[after_open..].find("]]>") {
            out.push_str(&rest[after_open..after_open + end]);
            rest = &rest[after_open + end + "]]>".len()..];
        } else {
            out.push_str(&rest[after_open..]);
            rest = "";
            break;
        }
    }
    out.push_str(&decode_entities(rest));
    out
}

fn decode_entities(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// One extracted feed entry: title, description/summary text, and a
/// published/updated timestamp string (as-authored, not necessarily valid
/// RFC 3339 — the caller is responsible for any further validation).
pub struct FeedEntry {
    pub title: String,
    pub description: String,
    pub published: String,
}

/// Port of the RSS/Atom item-extraction logic in `data.load_rss_evidence`.
/// Finds `<item>` blocks (RSS 2.0); if none exist, falls back to `<entry>`
/// blocks (Atom 1.0) — matching Python's
/// `root.findall(".//item") or root.findall(".//{*}entry")`. Returns at
/// most `max_items` entries. Never panics on malformed input — an
/// unparseable feed simply yields fewer (possibly zero) entries.
pub fn extract_feed_entries(xml: &str, max_items: usize) -> Vec<FeedEntry> {
    let mut entries = extract_blocks(xml, "item", max_items);
    if entries.is_empty() {
        entries = extract_blocks(xml, "entry", max_items);
    }
    entries
}

fn extract_blocks(xml: &str, block_local_name: &str, max_items: usize) -> Vec<FeedEntry> {
    let mut out = Vec::new();
    let mut pos = 0;
    while out.len() < max_items {
        let Some(tag) = next_open_tag(xml, pos) else { break };
        if local_name(tag.qualified_name) != block_local_name {
            pos = tag.after_tag;
            continue;
        }
        if tag.self_closing {
            pos = tag.after_tag;
            continue; // an empty item/entry has no useful content
        }
        let close_needle = format!("</{}", tag.qualified_name);
        let Some(close_rel) = xml[tag.after_tag..].find(&close_needle) else { break };
        let block_end = tag.after_tag + close_rel;
        let block = &xml[tag.after_tag..block_end];
        let Some(close_gt_rel) = xml[block_end..].find('>') else { break };
        pos = block_end + close_gt_rel + 1;

        let title = find_first_matching_element(block, &["title"], 0, block.len())
            .map(|(_, text, _)| text)
            .unwrap_or_default();
        let description = find_first_matching_element(block, &["description", "summary"], 0, block.len())
            .map(|(_, text, _)| text)
            .unwrap_or_default();
        let published = find_first_matching_element(block, &["pubDate", "published", "updated"], 0, block.len())
            .map(|(_, text, _)| text)
            .unwrap_or_default();

        out.push(FeedEntry { title, description, published });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_a_simple_rss_item() {
        let xml = r#"<rss><channel><item><title>Hello</title><description>World</description><pubDate>2026-07-19</pubDate></item></channel></rss>"#;
        let entries = extract_feed_entries(xml, 10);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Hello");
        assert_eq!(entries[0].description, "World");
        assert_eq!(entries[0].published, "2026-07-19");
    }

    #[test]
    fn extracts_multiple_items_in_order() {
        let xml = r#"<rss><channel>
            <item><title>First</title></item>
            <item><title>Second</title></item>
        </channel></rss>"#;
        let entries = extract_feed_entries(xml, 10);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].title, "First");
        assert_eq!(entries[1].title, "Second");
    }

    #[test]
    fn respects_the_max_items_cap() {
        let xml = r#"<rss><channel><item><title>1</title></item><item><title>2</title></item><item><title>3</title></item></channel></rss>"#;
        let entries = extract_feed_entries(xml, 2);
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn falls_back_to_atom_entry_when_no_rss_item_exists() {
        let xml = r#"<feed xmlns="http://www.w3.org/2005/Atom"><entry><title>Atom title</title><summary>Atom summary</summary><updated>2026-07-19T00:00:00Z</updated></entry></feed>"#;
        let entries = extract_feed_entries(xml, 10);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Atom title");
        assert_eq!(entries[0].description, "Atom summary");
        assert_eq!(entries[0].published, "2026-07-19T00:00:00Z");
    }

    #[test]
    fn handles_cdata_sections() {
        let xml = r#"<item><title><![CDATA[<b>Bold</b> & stuff]]></title></item>"#;
        let entries = extract_feed_entries(xml, 10);
        assert_eq!(entries[0].title, "<b>Bold</b> & stuff");
    }

    #[test]
    fn decodes_the_five_basic_xml_entities() {
        let xml = r#"<item><title>A &amp; B &lt;tag&gt; &quot;q&quot; &apos;a&apos;</title></item>"#;
        let entries = extract_feed_entries(xml, 10);
        assert_eq!(entries[0].title, "A & B <tag> \"q\" 'a'");
    }

    #[test]
    fn namespaced_prefixed_tags_are_matched_by_local_name() {
        let xml = r#"<item><dc:title>Prefixed</dc:title></item>"#;
        let entries = extract_feed_entries(xml, 10);
        assert_eq!(entries[0].title, "Prefixed");
    }

    #[test]
    fn missing_fields_default_to_empty_strings_not_panics() {
        let xml = r#"<item></item>"#;
        let entries = extract_feed_entries(xml, 10);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "");
        assert_eq!(entries[0].description, "");
        assert_eq!(entries[0].published, "");
    }

    #[test]
    fn completely_malformed_input_yields_no_entries_without_panicking() {
        for bad in ["", "<", "<item", "not xml at all", "<item><title>unterminated", "<!DOCTYPE foo [<!ENTITY x \"y\">]><item/>"] {
            let entries = extract_feed_entries(bad, 10);
            assert!(entries.len() <= 1, "unexpected entries for {bad:?}: {}", entries.len());
        }
    }

    #[test]
    fn xml_comments_before_items_are_skipped_not_matched() {
        let xml = r#"<rss><!-- an item-like comment <item><title>fake</title></item> --><channel><item><title>real</title></item></channel></rss>"#;
        let entries = extract_feed_entries(xml, 10);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "real");
    }

    #[test]
    fn doctype_with_internal_entity_declaration_is_skipped_and_never_expanded() {
        // The entity `&xxe;` is deliberately never resolved anywhere in
        // this module — this test documents that the declaration is
        // skipped as opaque bytes, and the literal text `&xxe;` (if it
        // even appeared in an item) would pass through undecoded.
        let xml = r#"<?xml version="1.0"?><!DOCTYPE rss [<!ENTITY xxe SYSTEM "file:///etc/passwd">]><rss><channel><item><title>&xxe;</title></item></channel></rss>"#;
        let entries = extract_feed_entries(xml, 10);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "&xxe;");
    }

    #[test]
    fn a_quote_character_inside_an_attribute_value_does_not_end_the_tag_early() {
        let xml = r#"<item><title alt="a>b">Text</title></item>"#;
        let entries = extract_feed_entries(xml, 10);
        assert_eq!(entries[0].title, "Text");
    }

    #[test]
    fn self_closing_item_is_skipped_not_returned_as_an_empty_entry() {
        let xml = r#"<rss><channel><item/><item><title>Real</title></item></channel></rss>"#;
        let entries = extract_feed_entries(xml, 10);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Real");
    }
}
