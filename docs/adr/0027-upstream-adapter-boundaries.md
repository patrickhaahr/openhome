# Upstream adapter boundaries

Axum talks to several third-party services (ECB, Open Food Facts, DTU Data,
OPTCG, TMDB, shops, RSS feeds). The adapters share a few rules that keep a
slow, hostile or chatty upstream from leaking into OpenHome or its logs.

- **One bounded read.** Each call is one request with a timeout and a byte cap
  that holds even when `Content-Length` is missing or wrong. It follows no
  redirects and makes no retries, so a rate limit is answered as unavailable
  rather than hammered. Only fields the domain needs are requested or trusted.
  DTU Data serves its workbook through a short redirect to its object store, so
  `food_database/import.rs` follows up to five, under a 64 MiB cap that leaves
  headroom over the 6.1 workbook (about 12.6 MB). The wishlist fetcher follows
  up to three, vetting each hop. The RSS feed fetcher breaks the cap rule: it
  buffers the whole body before checking its 2 MiB limit
  ([#335](https://github.com/patrickhaahr/openhome/issues/335)).
- **Opaque failures.** Every upstream failure becomes one `Unavailable`
  outcome. Upstream text is logged at most, never returned, so no provider
  message can reach a client or Hermes. Import failures say only that the food
  database is unavailable. RSS feeds are the exception: a refresh stores HTTP
  and parse errors in `last_error` and returns every error's text in the
  refresh summary ([#335](https://github.com/patrickhaahr/openhome/issues/335)).
- **Upstream-specific meaning stays in the adapter.** Open Food Facts v2
  answers an unknown barcode with `status: 0`, usually with a 404, and that
  means not found. The OPTCG catalogue answers an unlisted code with its own
  404 body (`{"error":"Card not found."}`); only that body is remembered as
  unlisted for the day, because a 404 from a proxy or a moved endpoint is an
  outage, not an answer. `docs/binder-values.md` covers the ECB parser and the
  exchange-rate job.
- **Identify politely, without personal data.** Open Food Facts requests name
  the app, its version and its public repository in the `User-Agent`, and
  carry no email address or other personal contact.
- **No server-side request forgery.** Feed URLs reject `localhost`,
  `*.localhost` and `*.local`, because they resolve to the host or the LAN.
  The wishlist fetcher goes further (ADR 0007, `docs/wishlists.md`): pinning
  the vetted DNS answers stops DNS rebinding. It sends no cookies, asks for
  `identity` encoding, and accepts only global unicast addresses, excluding
  special-use, documentation and 6to4 (`2002::/16`) ranges. No runtime setting
  bypasses these checks; only test builds map vetted demo hosts to loopback
  responders.
- **Private coordinates stay out of logs.** Raw request errors from the
  weather provider contain the coordinate-bearing URL, so they are not logged.
  A routing response may append elevation to each coordinate, and only
  longitude and latitude are kept.
