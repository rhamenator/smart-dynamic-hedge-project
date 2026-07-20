use std::time::Duration;

use serde_json::Value;
use smart_hedge_config::LoadedConfig;
use smart_hedge_models::{EvidenceItem, TimestampUtc};

use crate::rss_xml::{extract_feed_entries, FeedEntry};

const USER_AGENT: &str = "smart-dynamic-hedge/0.2 research reader";
const MAX_FEEDS: usize = 10;
/// Hardcoded, not configurable — matches Python's `load_rss_evidence`,
/// which passes a literal `timeout=8.0` to `urlopen` rather than reading
/// it from `provider.rss` (unlike Alpaca/FRED, `RssConfig` has no
/// `timeout_seconds` field at all).
const REQUEST_TIMEOUT_SECS: f64 = 8.0;

/// A minimal `netloc` (authority component) extractor for display/labeling
/// purposes only — not a security-relevant parse, so a hand-rolled
/// approximation is fine (unlike TLS, this has no adversarial-input
/// consequence if it gets an unusual URL slightly wrong).
fn netloc(url: &str) -> String {
    let after_scheme = match url.find("://") {
        Some(i) => &url[i + 3..],
        None => url,
    };
    let end = after_scheme.find(['/', '?', '#']).unwrap_or(after_scheme.len());
    after_scheme[..end].to_string()
}

fn error_evidence(feed_index: usize, url: &str, reason: &str) -> EvidenceItem {
    EvidenceItem {
        evidence_id: format!("rss-error-{feed_index}"),
        kind: "data_quality".to_string(),
        title: "RSS retrieval error".to_string(),
        timestamp: TimestampUtc::now().to_iso_string(),
        source: format!("rss:{}", netloc(url)),
        value: Value::Null,
        text: reason.to_string(),
        quality: 1.0,
        untrusted_text: false,
    }
}

/// Port of the per-item `EvidenceItem` construction in
/// `data.load_rss_evidence`. Pure and directly testable, separated from
/// the network fetch. Replicates Python's exact `or`-then-`.strip()`
/// ordering for `title` (fall back to `"RSS item"` only when the raw
/// extracted text is empty, *then* trim whatever was chosen — so a
/// whitespace-only title ends up empty, not `"RSS item"`, matching
/// Python's `(x or "RSS item").strip()`) — `description`/`published` are
/// never trimmed, also matching Python.
fn entry_evidence(feed_index: usize, item_index: usize, symbol: &str, url: &str, entry: &FeedEntry) -> EvidenceItem {
    let chosen_title = if entry.title.is_empty() { "RSS item" } else { entry.title.as_str() };
    let title = chosen_title.trim();
    let published =
        if entry.published.is_empty() { TimestampUtc::now().to_iso_string() } else { entry.published.clone() };
    let combined_title = format!("{symbol}: {title}");
    EvidenceItem {
        evidence_id: format!("rss-{feed_index}-{item_index}"),
        kind: "news".to_string(),
        title: combined_title.chars().take(240).collect(),
        timestamp: published,
        source: format!("rss:{}", netloc(url)),
        value: Value::Null,
        text: entry.description.chars().take(5000).collect(),
        quality: 0.45,
        untrusted_text: true,
    }
}

fn fetch_feed(url: &str, timeout: Duration) -> Result<String, String> {
    let response = ureq::get(url).set("User-Agent", USER_AGENT).timeout(timeout).call().map_err(|e| e.to_string())?;
    // Matches Python's `response.read(2_000_000)` cap — see `http_util`.
    // The highest-value place for this bound of the three: an RSS feed
    // URL is arbitrary operator configuration, not a fixed trusted host.
    crate::http_util::read_capped_body(response, 2_000_000)
}

/// Python's `list(rss.get("feeds", []))[:10]` — a pure slice, split out so
/// the cap is testable without a real network call.
fn capped_feeds(feeds: &[String]) -> &[String] {
    &feeds[..feeds.len().min(MAX_FEEDS)]
}

/// Port of `data.load_rss_evidence`. Never returns an error — a fetch
/// failure becomes a `data_quality` evidence item, matching Python's
/// `except Exception`. **Deviation from Python**: Python's exception
/// handler also catches XML *parse* failures (`ET.fromstring` raising) and
/// reports those as the same kind of error evidence; this crate's
/// `extract_feed_entries` never errors on malformed XML by design (see
/// `rss_xml`'s module doc) — it degrades to fewer/zero entries instead —
/// so a feed that fetches successfully but isn't valid feed XML silently
/// yields no evidence here rather than an `rss-error-*` item. Recorded as
/// an intentional, documented behavioral difference, not an oversight.
pub fn load_rss_evidence(loaded: &LoadedConfig, symbol: &str) -> Vec<EvidenceItem> {
    let rss = &loaded.config.provider.rss;
    if !rss.enabled {
        return vec![];
    }
    let max_items = rss.max_items_per_feed.clamp(0, 20) as usize;
    let timeout = Duration::from_secs_f64(REQUEST_TIMEOUT_SECS);

    let mut output = Vec::new();
    for (feed_index, url) in capped_feeds(&rss.feeds).iter().enumerate() {
        match fetch_feed(url, timeout) {
            Ok(raw) => {
                let entries = extract_feed_entries(&raw, max_items);
                for (item_index, entry) in entries.iter().enumerate() {
                    output.push(entry_evidence(feed_index, item_index, symbol, url, entry));
                }
            }
            Err(reason) => output.push(error_evidence(feed_index, url, &reason)),
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn netloc_extracts_the_host_from_a_full_url() {
        assert_eq!(netloc("https://example.com/feed.xml?x=1"), "example.com");
    }

    #[test]
    fn netloc_handles_a_url_with_no_path() {
        assert_eq!(netloc("https://example.com"), "example.com");
    }

    #[test]
    fn netloc_handles_a_url_with_a_port() {
        assert_eq!(netloc("http://example.com:8080/feed"), "example.com:8080");
    }

    #[test]
    fn netloc_falls_back_to_the_whole_string_with_no_scheme() {
        assert_eq!(netloc("example.com/feed"), "example.com");
    }

    fn entry(title: &str, description: &str, published: &str) -> FeedEntry {
        FeedEntry { title: title.to_string(), description: description.to_string(), published: published.to_string() }
    }

    #[test]
    fn entry_evidence_prefixes_the_title_with_the_symbol() {
        let item = entry_evidence(0, 0, "SPY", "https://example.com/feed", &entry("Big News", "details", "2026-07-19"));
        assert_eq!(item.title, "SPY: Big News");
    }

    #[test]
    fn entry_evidence_defaults_a_missing_title_to_rss_item() {
        let item = entry_evidence(0, 0, "SPY", "https://example.com/feed", &entry("", "details", "2026-07-19"));
        assert_eq!(item.title, "SPY: RSS item");
    }

    /// Matches Python's `(x or "RSS item").strip()`: a whitespace-only
    /// title is truthy in the `or`, so it's chosen and then stripped to
    /// empty — the `"RSS item"` fallback never gets a chance to apply.
    #[test]
    fn entry_evidence_whitespace_only_title_ends_up_empty_not_rss_item() {
        let item = entry_evidence(0, 0, "SPY", "https://example.com/feed", &entry("   ", "details", "2026-07-19"));
        assert_eq!(item.title, "SPY: ");
    }

    #[test]
    fn entry_evidence_defaults_a_missing_published_date_to_now() {
        let item = entry_evidence(0, 0, "SPY", "https://example.com/feed", &entry("t", "d", ""));
        assert!(!item.timestamp.is_empty());
        assert_ne!(item.timestamp, "");
    }

    #[test]
    fn entry_evidence_truncates_title_to_240_and_text_to_5000_chars() {
        let long_title = "x".repeat(500);
        let long_desc = "y".repeat(6000);
        let item = entry_evidence(0, 0, "SPY", "https://example.com/feed", &entry(&long_title, &long_desc, "t"));
        assert_eq!(item.title.chars().count(), 240);
        assert_eq!(item.text.chars().count(), 5000);
    }

    #[test]
    fn entry_evidence_marks_untrusted_text_true_and_lower_quality() {
        let item = entry_evidence(0, 0, "SPY", "https://example.com/feed", &entry("t", "d", "2026-07-19"));
        assert!(item.untrusted_text);
        assert_eq!(item.quality, 0.45);
    }

    #[test]
    fn error_evidence_is_kind_data_quality_and_trusted() {
        let item = error_evidence(2, "https://example.com/feed", "Transport");
        assert_eq!(item.kind, "data_quality");
        assert!(!item.untrusted_text);
        assert_eq!(item.evidence_id, "rss-error-2");
    }

    #[test]
    fn feed_list_is_capped_at_ten() {
        let feeds: Vec<String> = (0..15).map(|i| format!("https://example.com/{i}")).collect();
        assert_eq!(capped_feeds(&feeds).len(), MAX_FEEDS);
    }

    #[test]
    fn feed_list_shorter_than_the_cap_is_unaffected() {
        let feeds: Vec<String> = (0..3).map(|i| format!("https://example.com/{i}")).collect();
        assert_eq!(capped_feeds(&feeds).len(), 3);
    }

    /// Real end-to-end test: a local mock server serves a canned RSS 2.0
    /// feed, and `load_rss_evidence` makes a real HTTP request against it
    /// (real TCP, real `ureq` client code) and then runs the *real*
    /// `rss_xml::extract_feed_entries` parser against the *real* response
    /// body — not a hand-built in-memory string like `rss_xml`'s own unit
    /// tests use.
    #[test]
    fn load_rss_evidence_makes_a_real_http_round_trip_and_parses_the_response() {
        let feed_xml = r#"<?xml version="1.0"?>
            <rss><channel>
                <item>
                    <title>Fed holds rates steady</title>
                    <description>The Federal Reserve left rates unchanged.</description>
                    <pubDate>Sun, 19 Jul 2026 20:00:00 GMT</pubDate>
                </item>
            </channel></rss>"#;
        let port = crate::mock_http_test_support::start(vec![("/feed.xml", (200, "application/rss+xml", feed_xml.to_string()))]);
        let feed_url = format!("http://127.0.0.1:{port}/feed.xml");

        let dir = std::env::temp_dir().join(format!("smart-hedge-data-rss-e2e-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("config.json");
        std::fs::write(
            &config_path,
            format!(r#"{{"provider": {{"rss": {{"enabled": true, "feeds": ["{feed_url}"], "max_items_per_feed": 3}}}}}}"#),
        )
        .unwrap();
        let loaded = smart_hedge_config::load_config(Some(&config_path), &smart_hedge_config::EnvOverrides::default(), &dir).unwrap();
        std::fs::remove_dir_all(&dir).ok();

        let evidence = load_rss_evidence(&loaded, "SPY");
        assert_eq!(evidence.len(), 1);
        assert_eq!(evidence[0].title, "SPY: Fed holds rates steady");
        assert_eq!(evidence[0].text, "The Federal Reserve left rates unchanged.");
        assert!(evidence[0].untrusted_text);
    }

    /// Same real HTTP path, but the mock feed server returns a non-2xx
    /// status — confirms the fetch-failure-becomes-evidence behavior
    /// survives a real transport round trip.
    #[test]
    fn load_rss_evidence_reports_a_real_http_error_as_evidence() {
        let port = crate::mock_http_test_support::start(vec![("/feed.xml", (404, "text/plain", "gone".to_string()))]);
        let feed_url = format!("http://127.0.0.1:{port}/feed.xml");

        let dir = std::env::temp_dir().join(format!("smart-hedge-data-rss-e2e-err-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("config.json");
        std::fs::write(
            &config_path,
            format!(r#"{{"provider": {{"rss": {{"enabled": true, "feeds": ["{feed_url}"]}}}}}}"#),
        )
        .unwrap();
        let loaded = smart_hedge_config::load_config(Some(&config_path), &smart_hedge_config::EnvOverrides::default(), &dir).unwrap();
        std::fs::remove_dir_all(&dir).ok();

        let evidence = load_rss_evidence(&loaded, "SPY");
        assert_eq!(evidence.len(), 1);
        assert_eq!(evidence[0].kind, "data_quality");
        assert_eq!(evidence[0].evidence_id, "rss-error-0");
    }

    fn evidence_from_feed_body(body: String) -> Vec<EvidenceItem> {
        let port = crate::mock_http_test_support::start(vec![("/feed.xml", (200, "application/rss+xml", body))]);
        let feed_url = format!("http://127.0.0.1:{port}/feed.xml");

        let dir = std::env::temp_dir().join(format!("smart-hedge-data-rss-adversarial-{}-{port}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("config.json");
        std::fs::write(
            &config_path,
            format!(r#"{{"provider": {{"rss": {{"enabled": true, "feeds": ["{feed_url}"], "max_items_per_feed": 5}}}}}}"#),
        )
        .unwrap();
        let loaded = smart_hedge_config::load_config(Some(&config_path), &smart_hedge_config::EnvOverrides::default(), &dir).unwrap();
        std::fs::remove_dir_all(&dir).ok();

        load_rss_evidence(&loaded, "SPY")
    }

    /// A "give it a real workout" battery: deliberately malformed,
    /// oversized, or unusual fake RSS/Atom feeds, each served over a real
    /// local HTTP round trip. The only universal invariant is "never
    /// panic" — some cases legitimately yield zero evidence (a malformed
    /// feed degrades gracefully, per `rss_xml`'s documented design), others
    /// succeed with unusual-but-valid content.
    #[test]
    fn load_rss_evidence_survives_a_battery_of_adversarial_feeds() {
        let many_items: String = (0..2000)
            .map(|i| format!("<item><title>Item {i}</title><description>d{i}</description></item>"))
            .collect();

        let cases: Vec<(&str, String)> = vec![
            ("empty_body", String::new()),
            ("non_xml_garbage", "<html><body>this is not a feed</body></html>".to_string()),
            ("truncated_mid_tag", "<rss><channel><item><title>Unterminated".to_string()),
            ("no_channel_wrapper", "<item><title>Bare item, no rss/channel wrapper</title></item>".to_string()),
            ("many_items_two_thousand", format!("<rss><channel>{many_items}</channel></rss>")),
            (
                "unicode_and_emoji_content",
                "<rss><channel><item><title>🚀 日本語 tïtle</title><description>emoji déscription 🎉</description></item></channel></rss>".to_string(),
            ),
            (
                "cdata_with_embedded_markup",
                "<rss><channel><item><title><![CDATA[<script>alert(1)</script>]]></title></item></channel></rss>".to_string(),
            ),
            ("deeply_nested_unrelated_elements", "<a><b><c><d><e><f><g>nope</g></f></e></d></c></b></a>".to_string()),
        ];

        for (name, body) in cases {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| evidence_from_feed_body(body)));
            assert!(outcome.is_ok(), "case {name:?} PANICKED");
        }
    }

    /// Security-relevant workout case: a feed's `<!DOCTYPE>` declares an
    /// external entity pointing at a second, independent local server (a
    /// "canary") — the classic XXE-driven SSRF payload shape. This proves
    /// the guarantee holds even with a real, working HTTP client in the
    /// picture (unlike `rss_xml`'s own unit test, which only checks the
    /// parser's *output text*): the canary must never receive a
    /// connection, because nothing in this codebase ever reads a DTD
    /// entity declaration in the first place.
    #[test]
    fn load_rss_evidence_never_triggers_an_xxe_driven_ssrf_request() {
        use std::net::TcpListener;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let canary_hit = Arc::new(AtomicBool::new(false));
        let canary_listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let canary_port = canary_listener.local_addr().unwrap().port();
        {
            let canary_hit = Arc::clone(&canary_hit);
            std::thread::spawn(move || {
                for _stream in canary_listener.incoming().flatten() {
                    canary_hit.store(true, Ordering::SeqCst);
                }
            });
        }

        let feed_xml = format!(
            r#"<?xml version="1.0"?><!DOCTYPE rss [<!ENTITY xxe SYSTEM "http://127.0.0.1:{canary_port}/ssrf-canary">]><rss><channel><item><title>&xxe;</title></item></channel></rss>"#
        );
        let evidence = evidence_from_feed_body(feed_xml);

        // No event to wait on (we're proving an absence) — a short,
        // generous sleep gives a real SSRF attempt every chance to have
        // happened before we check.
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(!canary_hit.load(Ordering::SeqCst), "the RSS parser must never resolve/fetch an external DTD entity");
        assert_eq!(evidence.len(), 1);
        assert_eq!(evidence[0].title, "SPY: &xxe;", "the literal entity reference should pass through unresolved");
    }

    #[test]
    fn disabled_rss_returns_no_evidence_without_any_network_call() {
        let dir = std::env::temp_dir().join(format!("smart-hedge-data-rss-disabled-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, r#"{"provider": {"rss": {"enabled": false, "feeds": ["https://example.com/feed"]}}}"#).unwrap();
        let loaded =
            smart_hedge_config::load_config(Some(&path), &smart_hedge_config::EnvOverrides::default(), &dir).unwrap();
        assert!(load_rss_evidence(&loaded, "SPY").is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }
}
