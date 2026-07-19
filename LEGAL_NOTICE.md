# Smart Dynamic Hedge: Additional Legal Notices

This file contains additional terms that are intended to be compatible with
GPLv3 section 7. These notices do not remove freedoms granted by GPLv3.

1) Preservation of notices and attribution (GPLv3 section 7(b), 7(c))

- You must preserve copyright notices, license notices, and this file in
  source distributions of covered works.
- Modified versions must be clearly marked as modified, including date and
  maintainer identity in a reasonable place.

1) No origin misrepresentation (GPLv3 section 7(c))

- You may not falsely represent that an altered version is the original
  Smart Dynamic Hedge project.

1) Name and mark use (GPLv3 section 7(d), 7(e))

- This license does not grant trademark rights in project names, logos,
  or branding assets.
- Use of project names for factual reference (for example, "forked from") is
  permitted under applicable law.

1) Disclaimer emphasis for financial-research context (GPLv3 section 7(a))

- The software is provided for educational and research purposes only.
- Nothing in this repository is financial advice, investment advice, or a
  recommendation to buy or sell any security or derivative.
- Users and deployers are solely responsible for regulatory, compliance,
  suitability, market, and operational decisions.

1) Liability boundary for execution systems

- Brokerage, exchange, and order-routing capability is a real, intended
  part of the overall Smart Dynamic Hedge system. It is built and armed
  exclusively in the separate `trade-guard-mcp` repository; this
  repository contains no such code itself, by design, so that execution
  credentials never share a codebase with this repository's model-facing
  and evidence-ingesting components.
- Whether execution is provided by `trade-guard-mcp` or by a third-party
  integration, the deploying operator is solely responsible for regulatory,
  compliance, suitability, market, and operational decisions arising from
  that deployment.
- Original authors and contributors are not responsible for losses,
  compliance failures, or regulatory consequences arising from any such
  deployment.

If any provision in this file is found invalid in a jurisdiction, the
remaining provisions continue to apply to the fullest extent permitted by law.
