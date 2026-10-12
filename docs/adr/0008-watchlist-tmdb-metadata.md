# Watchlist metadata belongs to the private Axum TMDB adapter

Use TMDB's free non-commercial developer API for movie and whole-series search
and metadata. Reuse the existing reqwest client library and Watchlist service;
Wishlist's HTML URL preview adapter does not provide media search or stable
provider identity. Axum holds the optional `TMDB_READ_ACCESS_TOKEN`, selects
`movie` or `tv` endpoints from OpenHome's Media Kind, and bounds requests and
bodies without following redirects. Start forwards authenticated API operations
and only safe, parsed results reach the browser.

Provider identity is `(kind, tmdb_id)`, with a unique owner/kind/ID index. Provider
selection is optional on addition and is recorded in its immutable retry receipt.
Metadata enrichment updates only title, description, poster path and release date;
it never rewrites personal status, added date or completion date. A failed provider
read writes nothing, consumes no retry intention and leaves manual planning usable.

The dashboard deliberately searches on request rather than every keystroke. It
shows the required attribution and an unmodified approved TMDB logo from the
[official asset](https://www.themoviedb.org/assets/2/v4/logos/v2/blue_short-8e7b30f73a4020692ccca9c88bafe5dcb6f8a62a4c6bc55cd9ba82bb2cd95f6c.svg).
Poster URLs use the fixed public TMDB image origin and a validated single filename;
no provider credential appears in image requests. This retains the existing Axum
ownership boundary at the cost of storing a metadata snapshot until a user refreshes
it. We introduce no viewing history, episode progress, or inferred completion.
