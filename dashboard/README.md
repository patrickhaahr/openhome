# OpenHome dashboard

The production TanStack Start dashboard connects to Axum through its server.
The initial connection screen checks the authenticated `/api/health` route.
Axum continues to own domain rules, persistence and provider integrations.

## Private configuration

Set these environment variables **on the dashboard server at runtime**:

| Variable           | Value                                                                            |
| ------------------ | -------------------------------------------------------------------------------- |
| `OPENHOME_API_URL` | Your Axum API's HTTP(S) base URL, optionally including a deployment path prefix. |
| `API_KEY`          | The same bearer API key configured on Axum.                                      |

Neither variable has a default. Do not prefix them with `VITE_`, put them in
browser storage or commit their values. Supply them using the private deployment's
environment or process manager, and restart the dashboard after changes.
Development and preview recipes inherit the invoking process's environment;
they do not automatically load credentials from a dotenv file.

The browser receives only a connection outcome and check timestamp. The server
does not return its backend address, key, backend response bodies or raw errors.
It does not follow redirects and bounds health checks to four seconds. A
successful health check confirms Axum authentication and health, not the status
of every provider or device.

Connection outcomes distinguish setup needed, invalid setup, rejected API key,
unreachable Axum, an unhealthy/unexpected response and connected. **Check again**
runs a fresh server check without caching the previous response. Dates and times
use Danish formatting in Europe/Copenhagen.

Deploy within the private LAN/VPN boundary from ADR 0003. Serve the dashboard
over HTTPS for the eventual Safari microphone features. Household location,
identities and provider configuration belong in private runtime configuration;
there are no production demo coordinates or hostnames.

## Commands

Run from the repository root:

```sh
devenv shell -- just dashboard-install
devenv shell -- just dashboard-run
devenv shell -- just dashboard-check
```

`dashboard-check` formats, lints, typechecks, runs rendered connection tests and
builds the production application. The tests start TanStack Start and the actual
Axum health route behind its authentication middleware on isolated loopback
ports with explicit synthetic connection configuration. The Axum fixture does
not load household configuration, databases or provider services.
They cover missing/invalid configuration, successful authentication, rejected
credentials, transport failure and recovery, and unhealthy/malformed responses,
including checking that private test values do not enter rendered page data.

To review a connected screen using synthetic data, run
`devenv shell -- just dashboard-health-fixture-run`. It prints its temporary
loopback base URL. Start a separate dashboard process with that URL as
`OPENHOME_API_URL` and `synthetic-dashboard-api-key` as `API_KEY`. A different
synthetic key exercises credential rejection; stopping the fixture exercises
network failure. This fixture contains only health, uses the real API key layer,
and is not a production integration service.

`dashboard-build` builds the application; `dashboard-preview` serves that build
for local review. The production runtime still needs the private environment
variables even if they were present during the build.

## Release verification

Physical verification remains required on the iPad 6th generation with
iPadOS 17.7.11 Safari before release. Desktop viewport checks do not establish
Safari or hardware compatibility. On the target device, verify landscape and
portrait layout, text zoom and scrolling, touch retry feedback, keyboard focus,
reduced motion, and recovery after Axum/network failure. Record the actual
device result before marking the parent release's device gate complete.

Public screenshots and fixtures must use anonymous synthetic data. Implementation
of this connection slice does not complete the shell, feature integrations or
the release tracked by issues #70 and #69.
