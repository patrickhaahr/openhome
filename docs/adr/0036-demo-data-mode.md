# Demo data mode

The Dashboard and the TV launcher can show believable health and workout data on
a device that has none, for filming. The data is the synthetic preview's
(ADR 0019), never a copy of household records.

- **Recorded, not generated.** `just dashboard-demo-snapshot` runs the synthetic
  API with `OPENHOME_PREVIEW_COMPLETE_TODAY=true`, so today holds a whole day, and
  records every health and workout response the Dashboard can request into
  `dashboard/src/lib/demo/snapshot.json`. The file is exempt from the file-size
  guard because it is generated data. The recorder refuses a capture that crosses
  a Copenhagen midnight and any epoch number or datetime that is not a plain
  Copenhagen-offset string, so the date shift stays exact.
- **One seam.** `requestOpenHome` answers health and workout GETs from the
  snapshot when the device cookie says `demo`, and everything else reaches the
  real API. Request dates shift back and response dates forward by whole days
  with the Copenhagen wall clock kept.
- **Read-only.** Every write in demo mode is refused with HTTP 405, so filming can
  never reach the household database or leave demo records behind.
- **Per device.** The `openhome-data` cookie holds `live` or `demo`. The Connection
  page switches it, and `?demo=on` or `?demo=off` on any URL redirects to the same
  URL without the parameter and without caching. There is no banner.
- **Known gaps.** Custom body date ranges, dates more than about 96 days back and
  exercises outside the library have no snapshot entry and answer 404. An event
  later in the day than the real clock shows as if it had already happened.
- **TV.** `just tv-demo-start` swaps the TV client for a bridge that answers only
  today's recovery from the snapshot and forwards everything else through the real
  client. `just tv-demo-stop` restores the client exactly as the preview capture
  does. The start leaves `client-disabled` in place, so a reboot keeps the real
  client off until the stop recipe runs.
