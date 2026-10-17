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

Bound work to 1–24 selected albums, 60 photo identities per page, 8 MiB per album
response (at most 20,000 upstream assets), 2 MiB per thumbnail and 12 MiB per preview.
The cap was raised from 8 to 24 so the household can add albums by appending an ID; every
other bound is per album or per page and is unchanged. A shared 30-second metadata cache
coalesces requests and retains only sanitized identities, names and capture times. This
permits up to 30 seconds of membership staleness; expired metadata is removed before
fetching and provider failure cannot serve it stale.
Four provider image reads run at once, with a five-second queue timeout and a
five-second provider timeout. There is no image cache; responses use `no-store`.

Full capture times (Immich `localDateTime`) never leave Axum. They order an **All photos** view
that merges the selected albums newest first, 60 identities per page, de-duplicated by
asset; each item names the selected album that grants access, so media reads keep the
same membership check. An album exposes only the first and last month (`YYYY-MM`) of its
photos, and the owner's chosen cover when it is a served still. The gallery loads further
pages as it scrolls; the full-screen dialog renders one preview at a time, steps across
loaded photos and reads the next page when stepping past the last.

Capture dates are the one exception, and only as a year and a month-day. "On this day"
(#197) needs to say which year a photo was taken in, and the idle slideshow favours
photos from the same month-day in earlier years, so `GET /api/photos/on-this-day?date=MM-DD`
returns, for one month-day, the photos captured on it in years before the current one:
each item's granting album, asset and capture year (with `years_ago`), newest year first.
Anyone who can read the endpoint can therefore learn the capture date of a photo it
returns, by asking day by day; the household approved this, since the dashboard is already
a trusted, private surface (ADR 0003) and the pictures themselves show far more. The time
of day, zone, EXIF, filenames, people and places still stay in Axum.

The capture day is local: the photo's own wall clock when Immich knows its zone (it then
writes that clock in `localDateTime`), otherwise the UTC instant read in Europe/Copenhagen,
where Immich writes the instant and a photo at 23:30 UTC in winter belongs to the next day.
The same local times order **All photos** and bound album months. A 29 February photo also
comes round on 28 February in common years. The endpoint reads the same cached album
metadata as the gallery and keeps its bounds: the selected albums only, 60 items per page,
no page past the last; malformed or impossible dates are rejected.

An explicitly enabled loopback preview can use albums approved for public evidence,
without connecting household databases or importing any other library content.
Normal production reads never default to public demo data. The idle slideshow and
its shared activity/draft/conversation protections remain in dependent issue #120.
