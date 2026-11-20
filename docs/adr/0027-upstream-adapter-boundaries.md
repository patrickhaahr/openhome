# Upstream adapter boundaries

Axum talks to several third-party services (ECB, Open Food Facts, DTU Data,
OPTCG, TMDB, shops, RSS feeds). The adapters share a few rules that keep a
slow, hostile or chatty upstream from leaking into OpenHome or its logs.

- **One bounded read.** Each call is one request with a short timeout and a
  byte cap that holds even when `Content-Length` is missing or wrong. It
  follows no redirects and makes no retries, so a rate limit is answered as
  unavailable rather than hammered. Only fields the domain needs are requested
  or trusted: from an ECB file only dates and the USD and DKK rates. DTU Data
  is the one exception to "no redirects": it serves its workbook through a
  short redirect to its object store, so `food_database/import.rs` follows a
  bounded number of them. Its byte cap is sized from the 6.1 workbook (about
  12.6 MB).
- **Opaque failures.** Every upstream failure becomes one `Unavailable`
  outcome. Upstream text is logged at most, never returned, so no provider
  message can reach a client or Hermes. Import failures say only that the food
  database is unavailable.
- **Upstream-specific meaning stays in the adapter.** Open Food Facts v2
  answers an unknown barcode with `status: 0`, usually with a 404, and that
  means not found. The OPTCG catalogue answers an unlisted code with its own
  404 body (`{"error":"Card not found."}`); only that body is remembered as
  unlisted for the day, because a 404 from a proxy or a moved endpoint is an
  outage, not an answer. ECB history files list days newest first under one
  `Cube`, and old days can lack DKK, so days without both rates are skipped.
- **Strict parsing of large files.** The ECB history file is roughly 8 MB on
  one line, so `services/ecb.rs` parses it as an event stream rather than
  loading a tree, and refuses a DTD.
- **Last good data wins.** The exchange-rate job writes nothing when the ECB is
  unreadable, so stored rates keep converting. It fetches the full history only
  when stored rates cannot reach the oldest stored price, otherwise the 90-day
  file, whose first day is at or before the newest stored one. A failed pass
  retries sooner than a day, so a startup before the network is up does not
  leave the household without rates for a day.
- **Identify politely, without personal data.** Open Food Facts requests name
  the app in the `User-Agent` and carry no contact detail.
- **No server-side request forgery.** Feed URLs reject `localhost`,
  `*.localhost` and `*.local`, because they resolve to the host or the LAN.
  The wishlist fetcher goes further (ADR 0007): it pins the exact vetted DNS
  answers, because resolving again allows DNS rebinding. It disables
  environment proxies, cookies, redirects and compression, and accepts only
  global unicast addresses, excluding special-use, documentation and 6to4
  (`2002::/16`) ranges. No runtime setting bypasses these checks; tests reach
  loopback responders by mapping vetted demo hosts. The fetched URL is kept
  rather than an untrusted `canonical` or `og:url` value.
- **The browser never contacts the provider.** Wishlist pictures are fetched
  by Axum, and only the item's own `image_url` is fetched. That address is
  never logged or echoed. TMDB posters accept a single validated filename, so
  the browser's fixed `image.tmdb.org` origin cannot be steered elsewhere.
- **Private coordinates stay out of logs.** Raw request errors from the
  weather provider contain the coordinate-bearing URL, so they are not logged.
  Routing logs carry status, provider code and error kind only. A routing
  response may append elevation to each coordinate, and only longitude and
  latitude are kept.
