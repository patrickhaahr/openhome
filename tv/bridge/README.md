# TV-local OpenHome client

`client.js` runs with the TV's Node 8.12. It connects to the Axum API through verified HTTPS,
using the Pi's private transport address while preserving `openhome.haahr.me` as Host/SNI.
Credentials stay in an owner-only `/var/lib/openhome-tv/client.json`:

```json
{"apiKey":"PRIVATE","transportHost":"100.120.132.42","serverName":"openhome.haahr.me"}
```

The local server binds **127.0.0.1:39479** only. Every request must have
`Host: 127.0.0.1:39479` and `X-OpenHome-TV: 1`. Any `Origin` header, request body or chunked
encoding is rejected. There are no CORS headers or generic proxy routes.

- `GET /state`: same-day weather and recovery display values. Weather follows Axum's validity;
  recovery refreshes after 60 seconds. Concurrent reads share a refresh. Failures retain only
  same-day values, explicitly stale; weather older than six hours is unavailable. A Copenhagen
  day change discards yesterday's values, including reads that cross midnight.
- `POST /command/lights-on`, `/command/lights-off`, `/command/speaker-bluetooth`,
  `/command/speaker-optical`: four exact actions, no body. Commands have one global in-flight
  slot and at most one start per second. HTTP 200 `{ "state": "accepted" }` acknowledges only
  that Axum accepted the request. It does not report physical device state. Commands never retry.

`/state` returns only:

```text
day
weather: state, temperature, code, high, low, rain, wind, fetchedAt, validUntil
recovery: state, charge, rest, effort, freshness, updatedAt
```

Weather state is `fresh`, `stale` or `unavailable`. Recovery state uses the same values for
transport freshness; its separate `freshness` field preserves NOOP's `confirmed`, `partial`,
`unconfirmed` or `unknown`. Measurements can be null; zero is an observed value. Recovery
figures come from Axum's three derived scores, including its Rest formula. Private identifiers,
coordinates, journals, raw errors and API credentials never enter the response.

API reads have a seven-second absolute deadline; commands have twelve seconds. Responses are
limited to 512 KiB. Redirects, malformed JSON, and command responses lacking a string `message`
are rejected. TLS verification stays enabled.

Tests: `node tv/bridge/test.js` from the repository root, through the repository's development
shell/TV check recipe. All test data is synthetic. Tests use an ephemeral loopback server and
a fake HTTPS transport; they never call the real API or control devices.
