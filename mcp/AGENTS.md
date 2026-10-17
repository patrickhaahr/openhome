# MCP guidance

- Watchlist tools add, list, read, correct status/dates and remove movie/whole-series plans through authenticated Axum. Require explicit owner targeting; missing owners return API clarification, never a guessed destination. Reuse request IDs/arguments after uncertain additions. Reads are read-only, deduplicated additions non-destructive/idempotent, corrections/removals destructive/idempotent. After an uncertain correction, read the explicit owner/entry destination before retrying. Axum owns completion defaults, date consistency and filters. See `../docs/watchlists.md`.

- Wishlist tools may create named lists and add, read, edit or delete items; forward typed authenticated API operations unchanged. Destination resolution, validation and deduplication belong in Axum. Reads stay read-only; annotate additive writes as non-destructive. Deduplicated additions and item edits/deletes are idempotent; edits/deletes are destructive. See `../docs/wishlists.md`.

- Important Date tools list, read, add, update and delete household-shared dates through authenticated Axum. Tags are explicit Household User IDs, never inferred; `person_ids` is required on addition, even when empty. Reuse request IDs/arguments after uncertain additions. Axum owns all day arithmetic. See `../docs/important-dates.md`.

- Read `../docs/training-context.md` before changing training tools or their contracts.
- Keep training tools read-only: each call makes one authenticated API GET and passes its JSON unchanged. Add an API endpoint before adding a tool that needs new interpretation; database access and domain rules belong in the API.
- Percent-encode path arguments, reject literal `.` and `..` segments, and keep API redirects disabled so requests stay on the intended endpoint.
