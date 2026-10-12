# Wishlist URL metadata is a preview before an addition

Axum provides destination-explicit, non-persistent product URL suggestions to
Dashboard and MCP. The caller reviews or corrects the suggestions, then uses the
existing addition receipt contract. Keeping extraction separate from writes
preserves manual entry on failures and avoids changing retry identity when a
product page changes. MCP forwards the operation without extracting metadata.

The metadata fetcher checks and pins all resolved public addresses for each hop,
disables proxies and automatic redirects, and bounds time, redirects and HTML
bytes. It does not reuse the existing Feed fetcher: that implementation checks
DNS before a separate HTTP resolution and cannot provide the pinned destination
or editable metadata contract. Scraper parses inert HTML instead of maintaining
a custom HTML tokenizer; no page scripts or credentials are used.
