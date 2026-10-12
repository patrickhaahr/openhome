# MCP guidance

- Watchlist tools add, list, read and remove manual movie/whole-series plans through authenticated Axum. Require explicit owner targeting; missing owners return API clarification, never a guessed destination. Reuse request IDs/arguments after uncertain additions. Reads are read-only, deduplicated additions non-destructive/idempotent, removals destructive/idempotent. See `../docs/watchlists.md`.

- Wishlist tools may create named lists and add, read, edit or delete items; forward typed authenticated API operations unchanged. Destination resolution, validation and deduplication belong in Axum. Reads stay read-only; annotate additive writes as non-destructive. Deduplicated additions and item edits/deletes are idempotent; edits/deletes are destructive. See `../docs/wishlists.md`.

- Read `../docs/training-context.md` before changing training tools or their contracts.
- Keep training tools read-only: each call makes one authenticated API GET and passes its JSON unchanged. Add an API endpoint before adding a tool that needs new interpretation; database access and domain rules belong in the API.
- Percent-encode path arguments, reject literal `.` and `..` segments, and keep API redirects disabled so requests stay on the intended endpoint.
