# TV remote verification (#93)

Home now offers the common LG TV commands, and the focused Controls view
(`/controls`) offers the full remote. Buttons come from the commands Axum
reports at `GET /api/ir`; commands are sent through the private TanStack server
to `POST /api/ir/lgtv`. A command acknowledgement stays separate from
**Power and volume: Not reported**. The speaker remote remains a later slice of #71.

## Public visual evidence

All captures use a synthetic OpenHome boundary and a synthetic server-held API
key. They contain no household devices, identities, provider addresses or
credentials.

| Before · 1280 × 800                     | After · 1280 × 800                         |
| --------------------------------------- | ------------------------------------------ |
| ![Home without a TV remote](before.png) | ![Home with common TV commands](after.png) |

| Controls · 1024 × 768                        | Controls portrait · 768 × 1024                       |
| -------------------------------------------- | ---------------------------------------------------- |
| ![Full TV remote in landscape](controls.png) | ![Full TV remote in portrait](controls-portrait.png) |

| Home portrait · 768 × 1024                     | Missing IR setup · 1024 × 768                                            | Stale command · 1024 × 768                                   |
| ---------------------------------------------- | ------------------------------------------------------------------------ | ------------------------------------------------------------ |
| ![Home TV card in portrait](home-portrait.png) | ![Missing IR setup without buttons, with Check again](missing-setup.png) | ![A command the remote no longer reports](stale-command.png) |

## Automated and browser checks

- `devenv shell -- just dashboard-check`: formatting, lint, types, tests and
  the production build pass.
- `devenv shell -- just api-test-integration ir`: all eight existing IR route
  tests pass. No API behavior changes were needed.
- Controlled HTTP responses verify authenticated discovery and JSON command
  dispatch with deployment URL prefixes. They also cover filtering of unknown
  reported commands, missing IR setup, an offline bridge, rejected credentials,
  malformed responses and commands the remote no longer knows (404). Provider
  text never reaches the browser.
- Rendered remotes verify that only reported commands appear, grouped on the
  full remote. Queued and pending taps send a single command, and accepted
  results never claim power or volume. Failure, RPC rejection and stale
  discovery recover through **Check again** without a page reload. Missing
  setup offers no buttons, and disconnected states pause commands.
- Discovery is deferred in the route loaders, so a slow IR bridge never delays
  Home or the lights.
- In a desktop browser against the synthetic boundary, three queued activations
  of **Volume up** dispatched one command. The pending announcement appeared,
  then the accepted result. A 404 command, missing IR setup and recovery all
  behaved as above. Landscape and portrait have no horizontal document overflow,
  and remote buttons are at least 56 CSS px high.

## Live household smoke — 11 October 2026

`devenv shell -- openhome ir status` returned the real IR remote's command list;
all 31 reported LG TV commands have dashboard buttons.

A local dashboard server then held the private connection. The Controls view
discovered all 31 commands from the real Axum API. One deliberately chosen
**Info** command, chosen because it only shows an on-screen banner, was sent
from the browser. The card showed **Info command accepted by OpenHome.** and
**Power and volume Not reported**. The server was stopped immediately afterwards.

No live command was used as an automated fixture or public capture. The TV's
response was not independently observed and remains unreported in the UI.
Physical iPad Safari touch on the TV remote remains a manual release check.

This completes the TV remote slice; it does not close the parent controls
specification (#71) or dashboard release tracker (#69).
