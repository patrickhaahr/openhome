# Selected Immich albums

Configure all three in the private Axum runtime:

- `IMMICH_URL`: Immich server root, without `/api`; deployment prefixes are supported.
- `IMMICH_API_KEY`: key with `album.read` and `asset.view` read permissions.
- `IMMICH_ALBUM_IDS`: comma-separated UUIDs for 1–8 selected albums.

Leave all unset to disable photos. Partial/invalid configuration fails startup with
sanitized guidance. Keys and IDs belong in private runtime configuration, never in
source, screenshots or dashboard `VITE_*` variables. Album names are visible in the
shared dashboard. Metadata above the documented caps produces an unavailable state.

The dashboard keeps its existing `OPENHOME_API_URL` and `API_KEY` server settings.
Browse **Photos**, choose an album, select a tile, and use **Previous**, **Next** or
arrow keys within the current page. **Close photo** or Escape returns focus to the
tile. Gallery page controls browse more photos. **Refresh photos** rereads metadata
(up to 30 seconds cached) and retries failed thumbnails; **Retry photo** retries a
failed full-screen preview. Empty albums and removed media never fabricate content.

Authenticated Axum reads:

- `GET /api/photos/albums`
- `GET /api/photos/albums/{album}?page=0` (60 identities per page)
- `GET /api/photos/albums/{album}/assets/{asset}/thumbnail`
- `GET /api/photos/albums/{album}/assets/{asset}/preview`

Media access requires both a selected album and asset membership. Responses are
not publicly cached. Originals, videos, uploads, library management and sharing
are outside this slice. The idle slideshow is separate work in #120.

For approved public screenshots only, set `IMMICH_PUBLIC_DEMO=true` and point the
same private configuration at approved demo album IDs, then run
`devenv shell -- just dashboard-photos-preview`. This smoke-tests every resized
image on the first page and serves only health/photos on a random loopback port,
using `synthetic-dashboard-api-key` for the local OpenHome API. Set the dashboard's
server URL to that printed port; it never connects a household database. The flag
has no effect on production. Never enable the preview for unapproved albums.
