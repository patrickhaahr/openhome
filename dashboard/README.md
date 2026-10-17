# OpenHome dashboard

The production TanStack Start dashboard connects to Axum through its server.
Home shows the live clock/date and links to focused feature views. Connection
at `/connection` checks the authenticated `/api/health` route.
Axum continues to own domain rules, persistence and provider integrations.

Current weather on Home and the seven-day forecast at `/weather` use Axum's
shared Open-Meteo cache. Configure `WEATHER_LATITUDE` and `WEATHER_LONGITUDE`
privately on the API. See [weather verification and setup](docs/weather/README.md).

## Shell and navigation

The persistent icon-above-label rail opens Home, Weather, Controls, Travel,
Workout, Lists, Photos, Server, Agent and Connection. These are
URL-backed routes with browser history and deep links. Lists holds the Wish and
Watch tabs (`/lists?tab=wish|watch`). On smaller or zoomed
viewports the rail becomes a horizontally scrollable top bar; the content
remains vertically scrollable. Navigation moves focus to the rendered page
heading, and a skip link goes directly to the content.

Home's clock uses Danish formats in Europe/Copenhagen, regardless of the
browser's time zone. It hydrates from the server-rendered time, updates at
minute boundaries, stops its timer while hidden and catches up on visibility.
It does not poll the API. Theme tokens and shared shadcn cards/buttons provide
charcoal surfaces, yellow accents, touch targets and reduced-motion support.

Feature pages currently say **Not available yet**, with specific explanations.
They do not infer provider configuration from authenticated API health or
display sample records as connected data. Later feature slices replace these
surfaces with Axum-backed behavior; the integrated Home belongs to #133.
Connection's detailed setup and failure states remain available independently.
Automatic data refresh/reconnection and draft preservation belong to #119.

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
Bun also loads a local `.env` when starting the production server directly.
For Docker deployment, Compose supplies the environment as described below.

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

`dashboard-check` formats, lints, typechecks, runs rendered behavior tests and
builds the production application. The tests start TanStack Start and the actual
Axum health route behind its authentication middleware on isolated loopback
ports with explicit synthetic connection configuration. The Axum fixture does
not load household configuration, databases or live provider services. Its local
synthetic TMDB upstream exercises production search and enrichment without credentials.
They cover missing/invalid configuration, successful authentication, rejected
credentials, transport failure and recovery, and unhealthy/malformed responses,
including checking that private test values do not enter rendered page data.
Rendered router tests cover card/sidebar navigation, browser history, destination
focus and honest unavailable states. The clock test checks Copenhagen midnight
rollover and visibility catch-up; an HTTP deep-link test covers server rendering
and serialization of the shell and feature page.

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

## Docker deployment

The React 19 application builds with Nitro's Bun preset and runs on Bun 1.4.2.
The runtime image contains only `.output`, runs as the `bun` user, listens on
port 3000, and checks `/connection` for HTTP liveness. Connection status still
reports Axum failures separately; an HTTP health check does not prove that every
integration is configured.

Use the shared stack in `api/compose.yml`. Keep the private `.env` beside that
file, using `api/.env.example` as a template. Its existing `API_KEY` is shared
by the API, MCP and dashboard; Compose passes only that key and the internal
`http://openhome-api:8000` URL to the dashboard. No credential file is copied
into the image or mounted into the container. The Docker build context excludes
all environment files.

Build and publish alongside the other services:

```sh
devenv shell -- just dashboard-docker-push 0.1.1
devenv shell -- just api-docker-push 0.6.0
devenv shell -- just mcp-docker-push 0.3.0
```

These use the existing `openhome-publisher` Buildx builder, which needs both
amd64 and arm64 support (native builder nodes or binfmt emulation). Each command
publishes the version tag and `latest`. `dashboard-docker-build` builds without
pushing, matching the API/MCP recipes; `dashboard-start` serves a local
production build.

Run `devenv shell -- just dashboard-docker-smoke <image>` against a locally
loaded or published image before deployment. It starts an isolated container
with synthetic credentials and an offline API, then checks rendered HTML and
every referenced CSS/JavaScript asset. Development-server tests cannot verify
the packaged production assets. Tailwind scans only `src`, so generated output
cannot change its stylesheet between the client and SSR builds.

On the Pi, install `api/compose.yml` as `~/d/openhome/compose.yml`, retaining its
private `.env` and `data/`. The stack uses `latest`; after publishing updates:

```sh
devenv shell -- just openhome-deploy
```

The recipe pulls all three images and waits for healthy containers. Before API
upgrades, back up both SQLite databases using SQLite's online backup mechanism
or stop the API before copying the full data directory. Keep the previous image
digests and Compose file for rollback; database migrations may require restoring
the matching backup.

Caddy can serve the dashboard on the API's existing hostname. Keep `/api/*`
routed to `openhome-api:8000` and `/mcp*` to `openhome-mcp:8001`, then route
the remaining paths to `openhome-dashboard:3000`. Restrict dashboard paths,
including its server functions, to LAN/VPN clients: the shared display does not
have a separate login. The existing `caddy` network provides container discovery;
no dashboard port needs to be published on the host.
See [the OpenHome Caddy site block](Caddyfile.example) for the shared-host setup;
replace only that block in the private Caddyfile, validate it, then reload Caddy.
If an editor replaced the bind-mounted file, Caddy may still see the old inode:
compare the host and container file checksums and recreate the Caddy container
with `docker compose up -d --no-deps --force-recreate caddy` if they differ.

## Release verification

Physical verification remains required on the iPad 6th generation with
iPadOS 17.7.11 Safari before release. Desktop viewport checks do not establish
Safari or hardware compatibility. On the target device, verify landscape and
portrait layout, text zoom and scrolling, touch retry feedback, keyboard focus,
reduced motion, and recovery after Axum/network failure. Record the actual
device result before marking the parent release's device gate complete.

Public screenshots and fixtures must use anonymous synthetic data. The shell
implements #91; its browser evidence and outstanding physical checks are in
[the shell verification record](docs/home-frame/README.md). Feature integrations
and the release tracked by issues #70 and #69 remain incomplete.

## Synthetic preview for UI work

Design work never uses the real household API. Run:

```sh
just dashboard-synthetic
```

It builds and starts a synthetic Axum API (`api/examples/dashboard_preview`, on
`127.0.0.1:3101`, override with `OPENHOME_PREVIEW_API_PORT`) and the dashboard
dev server on <http://localhost:3001/>, wired to it with a synthetic key. Ctrl-C
stops both. `just dashboard-synthetic-api` runs the API alone.

The API runs the production routes, migrations and authentication over
in-memory databases, so responses always match the real contract. Loopback
stand-ins replace the upstreams: openrouteservice, Open-Meteo (central
Copenhagen demo location), Immich (generated landscapes, no photographs), the IR
bridge (full TV and speaker command sets) and the SwitchBot bridge. TMDB is the
exception: when `TMDB_READ_ACCESS_TOKEN` is set, or a token file exists at
`~/.config/openhome/tmdb-read-access-token`, Watchlist search uses the real TMDB;
otherwise a loopback catalogue of the seeded titles answers. Every page
renders populated: about 40 days of recovery, sleep stages, journal, naps and NOOP
workouts with heart rate ending today, five weeks of logged workouts, body
weight, Wishlists and the Watchlist. Dates are generated relative to today.
Wish pictures are drawn in `media.rs` and answered by a preview stand-in for the
authenticated wish picture route, because the production fetcher refuses
loopback addresses.

The invented values live in `api/examples/dashboard_preview/seed/*.json`
(`training`, `workouts`, `body`, `household`, `weather`, `photos`, `controls`);
edit them and restart. Sleep stage segments are generated in 90-minute cycles
in `noop.rs`, and daily totals are computed from them.

**Screenshots for pull requests and public evidence must come from this
synthetic preview**, never from the household API.
