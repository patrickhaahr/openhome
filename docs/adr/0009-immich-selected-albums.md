# Immich photos belong to a selected-album Axum adapter

Axum holds `IMMICH_URL`, `IMMICH_API_KEY` and an explicit `IMMICH_ALBUM_IDS`
allowlist. Reuse reqwest, UUID parsing and capability-owned route state rather
than adding database tables or allowing the browser to discover a private library.
The existing weather/TMDB adapters do not supply album membership or private media.
TanStack Start reuses its authenticated OpenHome transport and provides a same-origin
resized-media endpoint; the browser receives no provider origin, key or shared link.
The private dashboard continues to rely on the trusted-network access model in ADR 0003.

The installed Immich v2.7.5 supports album details and
`assets/{id}/thumbnail?size=thumbnail|preview`. These are resized still images;
we never request `original`/`fullsize` or follow provider redirects. Album membership
is checked before each image read. Strip original filenames, EXIF, people, paths
and owner information from metadata. Deleted albums/photos and setup/provider
failures remain explicit. Videos, trashed and offline assets are excluded.

Bound work to 1–8 selected albums, 60 photo identities per page, 8 MiB per album
response (at most 20,000 upstream assets), 2 MiB per thumbnail and 12 MiB per preview.
A shared 30-second metadata cache coalesces requests and retains only sanitized
identities/names. This permits up to 30 seconds of membership staleness; expired
metadata is removed before fetching and provider failure cannot serve it stale.
Four provider image reads run at once, with a five-second queue timeout and a
five-second provider timeout. There is no image cache; responses use `no-store`.
The full-screen dialog renders one preview at a time, with previous/next confined
to the current page and page controls available on return to the gallery.

An explicitly enabled loopback preview can use albums approved for public evidence,
without connecting household databases or importing any other library content.
Normal production reads never default to public demo data. The idle slideshow and
its shared activity/draft/conversation protections remain in dependent issue #120.
