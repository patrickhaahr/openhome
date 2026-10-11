# MCP guidance

- Wishlist tools may create named lists and add generic items; forward typed authenticated API operations unchanged. Destination resolution, validation and deduplication belong in Axum. Reads stay read-only; annotate additive writes as non-destructive and mark only deduplicated additions idempotent. See `../docs/wishlists.md`.

- Read `../docs/training-context.md` before changing training tools or their contracts.
- Keep training tools read-only: each call makes one authenticated API GET and passes its JSON unchanged. Add an API endpoint before adding a tool that needs new interpretation; database access and domain rules belong in the API.
- Percent-encode path arguments, reject literal `.` and `..` segments, and keep API redirects disabled so requests stay on the intended endpoint.
