# Selected Immich albums

Configure all three in the private Axum runtime:

- `IMMICH_URL`: Immich server root, without `/api`; deployment prefixes are supported.
- `IMMICH_API_KEY`: key with `album.read` and `asset.view` read permissions.
- `IMMICH_ALBUM_IDS`: comma-separated UUIDs for 1–24 selected albums.

Leave all unset to disable photos. Partial/invalid configuration fails startup with
sanitized guidance. Keys and IDs belong in private runtime configuration, never in
source, screenshots or dashboard `VITE_*` variables. Album names are visible in the
shared dashboard. Metadata above the documented caps produces an unavailable state.

## Add an album

1. Open the album in the Immich web app. Its address ends in `/albums/<uuid>`; copy
   that UUID.
2. Append it to `IMMICH_ALBUM_IDS` in the private API configuration, comma-separated:
   `IMMICH_ALBUM_IDS=<first-uuid>,<second-uuid>`. Order sets the order of the plates.
3. Restart the API. The album appears on **Photos** within 30 seconds of the restart.

The API key's owner must be able to read the album (`album.read`, `asset.view`). Remove
an ID the same way to hide an album; a deleted or unshared album shows as unavailable.

## Browse

The dashboard keeps its existing `OPENHOME_API_URL` and `API_KEY` server settings.
**Photos** shows each album as a cover plate with its photo count and months, led by
**All photos** (every selected album merged, newest first). With one album configured
the page opens it directly. An album is a justified wall that loads 60 more photos as
you scroll, with a **Load more** key as the fallback. Select a photo to open it full
screen; **Previous**, **Next**, the arrow keys or a swipe step through the photos, and
**Close photo** or Escape returns focus to the photo's tile. The open album and photo are
in the address, so Back and shared links work. **Retry photo** retries a failed
full-screen preview; **Try again** recovers an unavailable library or album. Empty
albums and removed media never fabricate content.

Authenticated Axum reads:

- `GET /api/photos/albums`
- `GET /api/photos/albums/{album}?page=0` (60 identities per page)
- `GET /api/photos/all?page=0` (every selected album, newest first, 60 per page)
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
