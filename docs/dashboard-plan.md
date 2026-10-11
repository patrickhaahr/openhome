# iPad dashboard

Status: needs-info

This is a discovery record, not an approved implementation specification. The
grill-with-docs interview is in progress. Recommendations remain proposals until
the user accepts them; implementation waits for shared understanding.

## Requested outcome

Use an old iPad as an OpenHome dashboard, starting from the `dashboard/` TanStack
Start template. Requested capabilities:

- Generate nearby walking and running routes, with targets such as 5 km, 10 km
  and 10,000 steps, and view routes on a map.
- Show live bus tracking associated with Buster and A-to-B transit planning
  associated with Rejseplanen.
- Show weather and a calendar, preferably connected to Proton.
- Build a new multi-user Wishlist integration inspired by the implementation of
  `cmintey/wishlist`, with OpenHome's own UI/UX; show photos from Immich.
- Build a per-user movie/series Watchlist with planned/watching/completed state,
  a date added to the plan and a completion date. This requires database
  migrations; dated comments and Jellyfin integration are under discussion.
- Expose home controls including lights, TV and speaker, and fitness features.
- Provide a conversational agent view with a reactive Jarvis-style avatar.
- Split features into separate implementation tickets as their scope is settled.

## Settled decisions — round 1

- Target device: iPad 6th generation, iPadOS 17.7.11. Real-device verification
  remains necessary; OS support alone does not establish graphics performance.
- A glanceable overview opens full-screen features on tap and returns home after
  inactivity. Idle photos are an accepted option; detailed behavior is open.
- Initial access is limited to the home network/private VPN, with one-time setup.
- Release includes every requested feature. Existing home controls are a sensible
  early implementation step, not a reduced release scope.
- Private runtime storage may contain personal data; explicitly selected external
  providers may receive the location needed to supply features.
- Public before/after PR evidence uses Immich demo content and an unrelated
  Copenhagen demo location. Synthetic calendar, shopping and fitness data must
  be used for other public evidence; never connect a public demo to private
  household services.
- Use GitHub issues and PRs rather than local Markdown implementation tickets.
  GitHub issues are enabled for `patrickhaahr/openhome`; no existing dashboard
  issue was found among the open issues inspected. This file is the evolving
  design record. Issue creation follows the shared-understanding review.

## Settled decisions — round 2

- Reuse existing OpenHome capabilities. AdGuard, Docker and feeds are low
  priority: the user reports that these API capabilities do not currently work
  correctly, so end-to-end acceptance depends on diagnosing and repairing them.
  This is a reported condition, not yet a reproduced defect. Do not assume a
  live API success from route presence or mocked responses.
- Axum owns integrations and business rules. TanStack Start serves the UI and
  keeps backend credentials server-side (ADR 0003).
- Route generation defaults to walking/running loops from a privately configured
  starting point, three alternatives, an adjustable step-length estimate, and a
  target distance tolerance of 10%, always displaying actual distance. Whether
  A-to-B walking is also required has not received a separate answer.
- A Google Earth-like map experience is desired as an add-on. Exact globe,
  satellite and terrain requirements, and whether it gates release, are open.
- Buster must supply live tracking within the dashboard. The user reports its
  app works without login and expects protocol/network investigation to discover
  the integration. A link to the app is not an accepted substitute. Access and
  feed compatibility are still unverified; absence of login does not establish
  an integration contract.
- Weather includes current conditions, hourly rain/temperature/wind and a
  seven-day forecast, emphasizing the next walking/running window.
- Calendar is read-only Proton agenda/week/month, with selected-calendar titles
  and times. The previously discussed feed delay is accepted; edits stay in
  Proton.
- Wishlist replaces the earlier interpretation of shopping as a checklist;
  whether a grocery list is additionally required remains open.
- Immich uses selected private albums for an idle slideshow and full-screen
  gallery, resized images, immediate return to controls on touch, and demo
  content for public PR evidence.
- Fitness includes logging/history, exercise progress and existing NOOP
  recovery/training summaries. A combined chart must show progress lines for
  all exercises, with individual exercise toggles. Cross-exercise units and
  comparison semantics remain open; do not fabricate upward trends.
- Reuse the existing continuously running Hermes instance as the agent brain.
  Conversation covers dashboard features and supported actions, with voice and
  text, initially push-to-talk, and an avatar reacting to listening, thinking,
  speaking and errors. Implement this feature last. Its private endpoint/host
  belongs in runtime configuration, never public planning artifacts.
- Display setup: plugged-in landscape, English UI with Danish local formats,
  dark styling with restrained Jarvis-like accents, large touch controls and
  configurable idle timing.
- Additional release feature: multi-user movie/series Watchlists, with planned
  and completed state, added and completion dates, backed by a migration.

## Settled decisions — round 3

- Household Users are configurable list owners without separate passwords.
  Wishlist and Watchlist show all users together; edits target the owner of the
  relevant list. The Dashboard has one shared view without a global profile
  selector. This is a trusted-household convention, not access enforcement.
- Only Wishlist and Watchlist become per-user initially. Existing fitness data,
  home controls and integrations keep their current ownership (ADR 0004).
- Each user has multiple named Wishlists. Items support title, URL, image, notes,
  quantity, priority and optional price; manual creation and editable URL-assisted
  metadata are supported. No gifting, reservations or purchased-gift features.
- The main Wishlist workflow is agent-driven: users supply a generic item or URL
  in a prompt, and the Agent adds it via OpenHome MCP tools. Explicit owner/list
  selection, generic-item enrichment and duplicate/retry behavior remain open.
- TMDB title/poster/description lookup is accepted, with manual titles available
  and OpenHome owning watch state. Watchlist adds a `watching` state for long
  series; dated season-progress comments and Jellyfin watch-state integration
  are being explored. Whole-series granularity, date corrections and no initial
  rewatch history were previously accepted, but integration-driven transitions
  now require additional decisions.
- The combined exercise chart uses per-exercise relative progress with selectable
  metrics, raw values on inspection, exercise toggles and truthful observed
  changes. Default metrics/normalization algorithms may be chosen on technical
  merit; do not invent a universal fitness score.
- Route handoff must be seamless if included: the user rejects a manual file
  export/transfer workflow and asks about an automatically selected route with
  live location in Expo. Keep/drop, foreground/background scope and an optional
  Signal screenshot have not been settled. No message sending is authorized.
- Map clarification: no globe. Walking/running and transit use a shared map
  concept with 2D/3D switching for the configured local region. Public demos use
  the agreed unrelated Copenhagen location. Exact 3D content and add-on release
  timing are still open.
- Provider cost constraint: no paid integrations/subscriptions. Free software
  and map data do not by themselves select a hosted tile/terrain/imagery service;
  choose supported free access or household-hosted services where necessary.
- Hermes uses dedicated dashboard conversations, reusing the existing brain.
- Avatar styles start with an orb/ring system; additional styles such as animated
  eyes are desired. Which styles gate initial release is open.

## Settled decisions — round 4 and prototype steering

- Include seamless Expo handoff: active-route retrieval and a foreground map
  with live location, caching route geometry locally. No manual file transfer.
  Background/screen-lock scope and selection changes mid-walk remain open.
- Generic Wishlist requests are recorded as generic wishes. Specific product
  research happens when asked; direct URLs trigger editable metadata lookup.
- An explicit Household User/list or private configured default provides the
  agent's destination. The shared Dashboard supplies no global active user.
  Hermes asks once when the destination is ambiguous.
- MCP includes Wishlist/list/item reads and writes and equivalent Watchlist
  tools. Existing training tools remain read-only. Wishlist MCP can ship before
  the dashboard voice/avatars, which stay last in implementation order.
- Jellyfin currently uses one shared household account. Its optional integration
  should attach shared viewing observations without inventing individual
  attribution. The user classifies it as nice-to-have rather than a release gate.
- Watching state and dated comments are wanted; automatic season/episode
  enrichment from Jellyfin is nice-to-have. Exact manual/sync transition rules
  remain open.
- Free 3D baseline accepted: pitched/extruded buildings using MapLibre and
  OpenFreeMap, with 2D/3D switching in walking/running and transit views.
- The 2D/3D switch and orb avatar gate initial release; other avatar styles such
  as animated eyes are later add-ons.
- Proton has no existing paid-plan entitlement. Investigate a free community MCP
  integration in the backlog; calendar availability no longer gates release.
  [proton-cal](https://github.com/cheeseandcereal/proton-cal) is a possible candidate,
  not a chosen or verified integration. It uses undocumented Proton APIs and
  needs compatibility verification with a free account.
- Dedicated Hermes conversation history persists privately, with new-conversation,
  stop and speech-interruption controls.
- The user now explicitly requests trying Executor v2 with a prototype. This
  authorizes an isolated experiment before the final production-plan review;
  it does not replace the agreed Axum ownership boundary or TanStack starter.
- Live voice is requested specifically through GPT-Live using the Codex
  subscription. Official documentation currently establishes subscription-backed
  Responses inference but not a subscription-backed Live voice entitlement.
  [GPT-Live](https://developers.openai.com/api/docs/models/gpt-live-1) is documented
  at $0.05/minute, with separate backend charges, while
  [subscription token-sharing limitations](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations)
  exclude audio/video inputs and transcription. No paid session is started;
  the user confirmed keeping zero fees, backlogging GPT-Live and investigating
  free voice alternatives.

## Executor prototype scope

The current prototype uses one shared household view, a compact icon-over-label
sidebar, black/charcoal surfaces and soft-yellow accents. Home separates time/date
from seven-day weather and shows four cards in two rows: controls and routes
above, exercise progress and bus arrivals below. A standalone clickable agent
orb opens its dedicated page. Wishlist, Watchlist, Photos and Calendar remain
separate sidebar pages. Public evidence uses synthetic household data and an
unrelated public Copenhagen map location; avatar states are demonstrative, not a
live GPT-Live/Hermes session. No private accounts or device commands are connected.

See [the prototype agent guide](../prototypes/executor-dashboard/AGENTS.md) and
[final page PNGs](../prototypes/executor-dashboard/screenshots/final/README.md) for
the implemented experiment, local workflow and remaining integration limits.

Executor runs locally with private runtime state ignored by Git. It supplies a
React SPA and app tools; its current UI host does not support TanStack Start
SSR. Prototype-only synthetic storage may be hosted by Executor, while the final
production source of truth remains Axum. The purpose is to assess UI iteration,
tool invocation and deployment reload, not silently migrate the product.

## Accepted handoff outline

Selecting a generated route would persist an Active Route in Axum. Expo would
retrieve it on launch/resume and while foregrounded, render its geometry, and
show the phone's current position. Cache the selected geometry for use away from
home; live position can remain on the phone rather than being uploaded. No
manual GPX export or transfer is required. The app must be reachable through
the agreed private network/VPN to receive a new selection.

Current code already refreshes adapters on app activation, but has no route API,
map view, selection persistence or continuous GPS stream. Selecting on the
dashboard does not itself launch a closed phone app. Notifications and
screen-lock/background tracking are separate scope decisions. Existing
GrapheneOS support means continuous GPS must work without relying on Google
Play services; the native AOSP location adapter currently handles only a single
position and geofencing.

A Signal screenshot could provide a visual reference or notification if later
requested. It is not an interactive route with live GPS. No Signal message is
being sent during design, and no delivery feature is yet approved.

## Confirmed constraints

- The repository is public. Do not commit personal locations or other PII.
- No hardcoded personal geographic locations.
- Do not include personal data or secrets in planning examples, tickets or
  research evidence.
- The template is present; the requested fast-forward pull completed and reported
  that the checkout was already up to date.

Personal locations belong in private runtime configuration/storage. Public demo
coordinates must be separate from the user's configured location.

## Existing system

- `CONTEXT.md` is the repository's domain glossary.
- Lights support on/off commands through SwitchBot; no light-state read exists.
- TV and speaker expose IR commands; observed power/volume state is not available.
- Workout logging and training read APIs exist. A logged Workout is distinct
  from a NOOP Workout.
- The API has no existing Home Assistant, routing, transit, weather, calendar,
  shopping or Immich integration.
- Agent currently means an operator using the CLI or read-only training tools;
  browser conversation and voice transport are new capabilities.
- Current API authorization uses a shared bearer API Key. The dashboard/backend
  boundary and trusted household-profile model are settled; session/setup
  implementation details remain open.
- Fitness currently has one global Profile and unowned Workouts, globally unique
  Exercise names and Body Weight dates. NOOP selects one active installation;
  its source/device identifiers are not household-user identities. Multi-user
  Wishlist/Watchlist requirements do not silently make fitness multi-user.
- `mcp/AGENTS.md` currently specifies exclusively read-only training tools, each
  forwarding a GET unchanged. Writable Wishlist tools require an explicit
  extension to this contract while preserving training tools' read-only behavior
  and Axum's ownership of integration/domain rules. The user requested MCP writes;
  transport/tool boundaries must be documented before implementation.
- Repository guidance defaults to local issues; the user's explicit instruction
  overrides that convention for this dashboard project: GitHub issues and PRs.

## Integration evidence

Official documentation reviewed on 2026-10-11. These findings constrain options;
they are not provider selections.

- [Buster](https://play.google.com/store/apps/details?id=dk.movia.buster) is
  Movia's live bus/Lokaltog tracking app for Zealand, Lolland, Falster and Møn.
  No documented public iframe or developer API was found. Embedding remains
  unverified; a departure board is not equivalent to live vehicle positions.
  [Lokaltog confirms no login or location sharing is needed](https://www.lokaltog.dk/om-os/presse/nyheder/foelg-din-bus-og-dit-lokaltog-live-med-buster/).
  Investigate normal app network traffic and anonymous-session behavior using
  public demo stops; sanitize fixtures before committing them. Authentication,
  rate limits, transport and endpoint stability are still unknown. An in-dashboard
  live-position adapter, rather than an app link, remains the release requirement.
- [Rejseplan API 2.0](https://labs.rejseplan.dk/hc/da/articles/21554723926557-Om-API-2-0)
  offers journey planning, nearby stops and departure boards. API 1.0 is closed;
  [access](https://labs.rejseplan.dk/hc/da/articles/21553113674909-Adgang-til-data-fra-Labs)
  requires approval, with a 50,000-call monthly noncommercial quota. Documented
  [deep links](https://labs.rejseplan.dk/hc/da/articles/21950431101085-Intelligente-links-til-rejseplan-dk)
  are another option. An official iframe contract was not found.
- [Proton calendar sharing](https://proton.me/support/share-calendar-via-link)
  offers read-only ICS links on paid plans. Full view exposes event details;
  limited view shows busy periods. The feed URL contains a decryption key and
  must be treated as a secret. Updates may take up to eight hours.
- [Immich's API](https://api.immich.app/) supports integration; credential scope,
  selected album and installed version need establishing. Public
  [sharing links](https://docs.immich.app/features/sharing/) grant access and
  should not appear in public artifacts.
- [Safari 15 introduced WebGL2](https://webkit.org/blog/11989/new-webkit-features-in-safari-15/).
  The target device/OS is now known, but rendering performance and voice/avatar
  behavior still require verification. MapLibre has a documented
  [WebGL support check](https://maplibre.org/maplibre-gl-js/docs/examples/check-if-webgl-is-supported/).
- Map rendering, OSM geographic data, basemap/satellite tiles and route generation
  are separate capabilities. [Openrouteservice round trips](https://giscience.github.io/openrouteservice/api-reference/endpoints/directions/routing-options)
  support preferred length and random seeds, with actual lengths potentially
  differing; this is a candidate provider, not a selection. Hosted versus
  self-hosted operation and spending limits remain open.
- [Immich's official demo](https://demo.immich.app/) is available for
  [development clients that do not upload photos](https://docs.immich.app/developer/setup/).
  Public previews should be read-only and must not assume demo uptime or the
  ability to create API credentials.
- The Wishlist reference separates owners, creators and claimants and supports
  named lists, groups, URL-assisted item entry and purchased/reserved gifts.
  [Schema](https://github.com/cmintey/wishlist/blob/main/prisma/schema.prisma)
  and [README](https://github.com/cmintey/wishlist#readme) are design inspiration,
  not requirements to replicate all features or reuse its UI. Manual item entry
  remains a viable fallback when URL metadata lookup fails. No source was copied.
- [TMDB movie search](https://developer.themoviedb.org/reference/search-movie)
  and [TV search](https://developer.themoviedb.org/reference/search-tv) are optional
  candidates for Watchlist metadata. [Application credentials](https://developer.themoviedb.org/docs/authentication-application)
  can be held server-side; noncommercial use requires
  [attribution](https://developer.themoviedb.org/docs/faq). Watch state belongs to
  OpenHome regardless of metadata provider; manual titles remain an option.
- Hermes supports [programmatic clients](https://hermes-agent.nousresearch.com/docs/developer-guide/programmatic-integration)
  and a [Runs HTTP/SSE API](https://hermes-agent.nousresearch.com/docs/user-guide/features/api-server)
  for streamed output, tool events, stopping and session continuity. Check the
  private deployment's version, capabilities and enabled transports before
  choosing its adapter. [Speech routes exist upstream](https://raw.githubusercontent.com/NousResearch/hermes-agent/main/hermes_cli/web_routers/audio.py)
  but are not assumed to be exposed by the same API server. Prefer relaying
  server-held credentials; do not embed the Hermes management dashboard.
- Jellyfin can expose per-user watched state, play count, last-played date and
  resume position ([official user-data model](https://github.com/jellyfin/jellyfin/blob/master/MediaBrowser.Model/Dto/UserItemDataDto.cs)).
  Match movie/TV entries by media kind plus provider ID
  ([item model](https://github.com/jellyfin/jellyfin/blob/master/MediaBrowser.Model/Dto/BaseItemDto.cs)).
  Current state is not a full past-viewing log, and last-played dates are not
  guaranteed first-completion dates. Map Household Users to private Jellyfin
  user IDs; sync direction, import scope and manual overrides remain open.
  [The official webhook plugin](https://github.com/jellyfin/jellyfin-plugin-webhook#documentation)
  can notify playback changes, while polling provides a plugin-free alternative.
- [Expo SDK 57 Location](https://docs.expo.dev/versions/v57.0.0/sdk/location/)
  distinguishes foreground position watching from background tasks.
  [MapLibre React Native](https://maplibre.org/maplibre-react-native/docs/setup/expo/)
  needs a native build. Expo's Android location implementation uses Google fused
  location ([upstream source](https://github.com/expo/expo/blob/sdk-57/packages/expo-location/android/src/main/java/expo/modules/location/LocationModule.kt));
  OpenHome's AOSP adapter needs extending to preserve no-Google-Play support.
- A concrete no-subscription baseline is available:
  [OpenFreeMap](https://openfreemap.org/) provides free hosted vector tiles,
  including the source in MapLibre's
  [3D building example](https://maplibre.org/maplibre-gl-js/docs/examples/display-buildings-in-3d/).
  Use flat 2D and pitched/extruded-building 3D as a proposed baseline. Satellite
  imagery and elevation are not supplied by OpenFreeMap; source-data building
  coverage/heights and sustained iPad performance require verification.
- [Open-Meteo's free terms](https://open-meteo.com/en/terms) permit personal home
  automation within noncommercial quotas.
  [Openrouteservice's FAQ](https://giscience.github.io/openrouteservice/frequently-asked-questions)
  documents Standard directions limits of 2,000/day and 40/minute. Use its
  current HeiGIT API documentation rather than hardcoding the deprecated
  `api.openrouteservice.org` host. These are proposed service choices and require
  operational quota/caching handling, not paid upgrades.

## Progress-chart design constraint

The existing per-exercise progress endpoint returns best reps, added weight,
volume, RPE and estimated 1RM, one Exercise per request. There is no batch read
or maximum hold-duration progress metric. Bodyweight Sets yield zero volume and
no estimated 1RM; Timed Holds yield no reps or estimated 1RM. The existing volume
calculation can also combine kg × reps and kg × seconds, so it is unsuitable as
an unquestioned common scale.

A single raw axis cannot compare kg, reps and seconds meaningfully. The accepted
comparison view indexes each Exercise to its own baseline and exposes the raw
measurement, chosen metric and baseline. Defaults use reps for bodyweight work,
seconds for Timed Holds and estimated 1RM for weighted repetitions; technical
refinement is delegated to implementation. Never-logged Exercises have no line,
missing observations are not invented, and declines remain visible. Any new
interpretation belongs in Axum and must preserve existing endpoint semantics.

## Decision tree

Round 1 resolved:

1. Hardware/browser target. Orientation/power arrangement remains open.
2. Glanceable overview with full-screen features. Detailed visibility is open.
3. Home-network/private-VPN access and one-time setup.
4. Complete release scope; existing home controls can be implemented first.
5. Public-artifact privacy, private runtime data and selected external providers.
6. GitHub issues and PRs.

Round 2 resolved: dashboard/backend boundary, primary feature semantics, display
behavior, Wishlist scope replacing the checklist proposal, extra Watchlist,
combined exercise chart and Hermes-last implementation order. New questions are
needed for multi-user scope, chart semantics and map/Buster acceptance.

Round 3 resolved: trusted Household Users, narrow per-user ownership, public
household lists, multiple Wishlists, no gifting, TMDB lookup, relative exercise
comparison, no paid providers, dedicated Hermes conversations and orb-first
avatar styles. Agent-first Wishlist tools, Jellyfin, watching state, seamless
mobile handoff and local 3D views extend the next frontier.

Round 4 resolved: foreground Expo handoff, generic-item behavior, agent owner/list
defaults, Wishlist/Watchlist tool scope, free 3D baseline, additional-avatar
backlog, dedicated conversation lifecycle, shared-account Jellyfin optionality
and Proton backlog. A prototype experiment is now explicitly authorized.

Remaining frontier: sidebar prototype review and Executor's final role; mobile
background behavior/active-route changes; shared Jellyfin import rules; no-fee voice feasibility; concrete household setup/transit/display defaults;
final release acceptance and GitHub issue/PR breakdown.

Dependent branches for later rounds:

- Hardware → map renderer/fallback, performance budget, voice feasibility.
- Display behavior → home screen, idle photos, navigation, refresh and stale data.
- Access/privacy → setup, credentials, runtime location, backend boundaries.
- Routes → starting point, loops versus A-to-B, step-to-distance estimate,
  distance tolerance, surface preferences, route selection and phone handoff.
- Transit → nearby departures versus moving vehicles, trip planning, provider
  access, native panels versus external links/embeds.
- Calendar → read versus write, event detail visibility, acceptable freshness.
- Wishlist → per-user ownership, item entry/metadata, sharing and reservations.
- Watchlist → movie/series metadata, state transitions, completion granularity,
  timestamps and per-user ownership.
- Photos → selected album, slideshow behavior and visibility.
- Home controls → existing OpenHome services versus actual Home Assistant
  integration, command feedback versus observed state.
- Fitness → Workout logging, training overview or both.
- Agent → conversation purpose, allowed actions, microphone activation,
  backend/provider, avatar states and conversation retention.
- Milestones → verifiable acceptance criteria, dependencies and individual
  tickets for each feature; final shared-understanding review.

Record answers as they arrive. Add settled domain terms to `CONTEXT.md`; use ADRs
only for consequential architectural choices with real trade-offs.

### Q42 — Voice funding

Confirmed: keep zero fees. GPT-Live is backlog, with free voice feasibility work. The prototype only simulates voice states and does not request microphone access or paid API sessions.

### Executor prototype outcome

The account-free [prototype](../prototypes/executor-dashboard/README.md) now runs in local Executor v2. Actual demo wishlist tools and persisted isolated SQL data work, alongside a public MapLibre/OpenFreeMap basemap. Deployment automatically reloads the open browser app. The initial Overview, Atlas and Companion variants informed the Q43–Q44 sidebar revision below. Home and the dedicated map page fit a 1024×768 Chromium viewport; physical iPad Safari remains unverified.

Beta14 required a system CA bundle for embedded workerd dependency downloads and a small replacement for MapLibre's CSS data-URL controls. No private account or paid service was used. Account connection convenience has not been tested. Executor's production role remains undecided: its browser app model does not directly host TanStack Start SSR, and final domain data/integration authority remains Axum under ADR 0003.

### Q43–Q44 — Sidebar and Home scope

Confirmed: the main dashboard prominently shows weather and home controls, with shortcuts to all other features. Use a persistent left sidebar for dedicated feature pages. Routes/transit use the full content area for maps; the agent has its own conversation page. Wishlist, Watchlist, Workout, Weather, Home Controls, Photos and Calendar each have a dedicated destination. Routes/transit share a map destination with route/transit modes in the prototype; exact final information architecture can be refined during implementation.

The sidebar revision replaces the earlier three-layout comparison. It remains an isolated Executor prototype; no production dashboard or integration is promoted by this UI decision.

### Visual direction — black and yellow

Confirmed: remove the greeting and displayed personal names. Use black surfaces with yellow accents, direct widgets and short functional labels. This replaces the initial green palette and conversational dashboard copy in the prototype.

Refinement: use charcoal layers and a restrained yellow accent for selection and primary actions. Neutral icons, badges and disabled controls avoid competing with those states. Remove the repeated visible Watchlist page title; the sidebar identifies the page. Keep a screen-reader heading for navigation and accessibility.

### Shared household view

Confirmed: remove the global profile selector. The Dashboard has one shared view; Wishlist and Watchlist show multiple users together within their pages. User sections make ownership visible and target additions and edits. Public prototype labels remain anonymous (User 1, User 2). Wishlist and Watchlist remain dedicated pages, without Home cards. This supersedes the earlier picker and selected-user agent context.
