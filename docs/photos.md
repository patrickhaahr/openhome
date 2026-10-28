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
**Close photo** or Escape returns focus to the photo's tile. When photos were taken on
today's date in earlier years, an **On this day** plate follows **All photos**; its photos
show the year they were taken. The open album and photo are
in the address, so Back and shared links work. **Retry photo** retries a failed
full-screen preview; **Try again** recovers an unavailable library or album. Empty
albums and removed media never fabricate content.

Authenticated Axum reads:

- `GET /api/photos/albums`
- `GET /api/photos/albums/{album}?page=0` (60 identities per page)
- `GET /api/photos/all?page=0` (every selected album, newest first, 60 per page)
- `GET /api/photos/on-this-day?date=MM-DD&page=0` (photos captured on that month-day in
  earlier years, newest year first, 60 per page; `date` defaults to today in Copenhagen)
- `GET /api/photos/albums/{album}/assets/{asset}/thumbnail`
- `GET /api/photos/albums/{album}/assets/{asset}/preview`

An on-this-day item carries only its granting album, asset, capture year and `years_ago`;
the capture day is the photo's local date (its own zone when Immich knows it, otherwise
Copenhagen), and 29 February photos also appear on 28 February in common years. See
ADR 0009 for why capture dates are exposed. Media access requires both a selected album and
asset membership. Responses are
not publicly cached. Originals, videos, uploads, library management and sharing
are outside this slice.

## Idle slideshow

After `IDLE_TIMEOUT_SECONDS` without a touch, click, key or wheel (default 300, five
minutes), the dashboard shows the selected albums full screen with the time, date and
today's weather and opted-in Important Dates on a small, half-clear night plate in the corner. Set it
on the **dashboard** server at
runtime: whole seconds from 10 to 86400, or `0` to turn the slideshow off; any other value
keeps the default.

- Photos crossfade slowly every 12 seconds with a gentle drift; with reduced motion they
  only crossfade. Photos close to the screen's shape fill it; others show whole.
- Photos from the first bounded on-this-day page lead each idle spell, with a quiet
  "3 years ago today" line on the plate only while such a photo is showing. No matches
  or a failed on-this-day read continues with the normal rotation. Both the day and
  photo choices refresh at Copenhagen midnight.
- The normal order is random but stable for the Copenhagen day: every page of every
  album is dealt in a seeded order and each page's photos shuffled the same way. Pages of 60
  identities are read only when reached, and only two preview images are held: the one
  showing and the next, preloading beneath it. The next idle spell carries on from where
  the last stopped.
- An unavailable library, network or a run of broken photos rests on the plain ground with
  the clock and tries again a minute later. There is no error screen.
- Open sheets and dialogs (Lists, the photo viewer, More), unsaved drafts, saves in flight
  and declared conversation activity hold the slideshow off; a block that appears while it
  shows wakes the dashboard.
- The page underneath stays mounted and inert, so waking returns to exactly the page,
  scroll and drafts that were showing. The first touch or key only wakes: the whole
  gesture (pointer down, up and the click, or the key) is swallowed and the screen stays on
  top for a moment after it, so no hidden control or device command is triggered.

For screenshots, `just dashboard-synthetic` sets `IDLE_PREVIEW=true`, which lets
`?idle=now` (for example `http://localhost:3001/?idle=now`) start the slideshow at once.
Without that server setting the parameter does nothing, so production ignores it.

For approved public screenshots only, set `IMMICH_PUBLIC_DEMO=true` and point the
same private configuration at approved demo album IDs, then run
`devenv shell -- just dashboard-photos-preview`. This smoke-tests every resized
image on the first page and serves only health/photos on a random loopback port,
using `synthetic-dashboard-api-key` for the local OpenHome API. Set the dashboard's
server URL to that printed port; it never connects a household database. The flag
has no effect on production. Never enable the preview for unapproved albums.
