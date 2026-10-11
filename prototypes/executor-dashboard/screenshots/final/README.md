# Final dashboard PNGs

Actual Chromium captures of the current Executor v2 prototype, taken 11 October 2026. All images use a 1024×768 CSS-pixel viewport except Home portrait (768×1024). These are the current visual references for all ten sidebar pages, plus Transit, the 3D route view and portrait Home.

The images contain synthetic household data and unrelated public Copenhagen map data. The wishlist retains previous demo test additions; no stored data was deleted for these captures. Photos and Calendar show their current unfinished integration states. Map pages and fonts depend on network access. Physical iPad/Safari verification is still pending.

Screenshots capture the visible viewport. Wishlist has 2 px of vertical overflow with the current demo entries; the transit overlay scrolls independently, so its lower integration-details control may be partly outside the visible area. These images are visual references, not evidence of live integrations or a feature-complete release.

See [AGENTS.md](../../AGENTS.md) for setup, deployment, privacy and recapture instructions. This folder is included in version control; temporary screenshots elsewhere remain ignored.

| View | URL page / UI selection | PNG |
| --- | --- | --- |
| Home | `home` | [home.png](home.png) |
| Routes | `travel` | [routes.png](routes.png) |
| Transit | `travel → Transit` | [transit.png](transit.png) |
| Weather | `weather` | [weather.png](weather.png) |
| Controls | `controls` | [controls.png](controls.png) |
| Workout | `workout` | [workout.png](workout.png) |
| Wishlist | `wishlist` | [wishlist.png](wishlist.png) |
| Watchlist | `watchlist` | [watchlist.png](watchlist.png) |
| Photos | `photos` | [photos.png](photos.png) |
| Calendar | `calendar` | [calendar.png](calendar.png) |
| Agent | `agent` | [agent.png](agent.png) |
| Routes in 3D | `travel → 3D` | [routes-3d.png](routes-3d.png) |
| Home in portrait | `home` | [home-portrait.png](home-portrait.png) |

## Home

Four cards in two rows; seven-day weather, time/date and standalone agent avatar.

![Home dashboard reference](home.png)

## Routes

2D public Copenhagen basemap and an illustrative loop; route generation is not connected.

![Routes dashboard reference](routes.png)

## Transit

Journey layout and sample bus estimates; Rejseplan and Buster are not connected.

![Transit dashboard reference](transit.png)

## Weather

Sample conditions and seven-day forecast.

![Weather dashboard reference](weather.png)

## Controls

Demo lights, TV and speaker controls; no real device commands.

![Controls dashboard reference](controls.png)

## Workout

Synthetic normalized exercise progress with independent line toggles.

![Workout dashboard reference](workout.png)

## Wishlist

Both anonymous users together; isolated Executor demo SQL data, including earlier test additions.

![Wishlist dashboard reference](wishlist.png)

## Watchlist

Both anonymous users, fictional titles and planned/watching/completed states; resets on reload.

![Watchlist dashboard reference](watchlist.png)

## Photos

Current Immich integration status page; not connected.

![Photos dashboard reference](photos.png)

## Calendar

Current Proton Calendar integration status page; backlog.

![Calendar dashboard reference](calendar.png)

## Agent

Idle orb and demo conversation tools; Hermes and live voice are not connected.

![Agent dashboard reference](agent.png)

## Routes in 3D

MapLibre building extrusions; a 3D map, not satellite imagery.

![Routes in 3D dashboard reference](routes-3d.png)

## Home in portrait

768×1024; the four Home cards still occupy two rows.

![Home in portrait dashboard reference](home-portrait.png)
