# Shared maps read a private Axum origin

The dashboard needs a local map before routes and transit are connected.
Existing weather configuration and device integrations were considered: none
owns a map viewport, and borrowing weather coordinates would couple independent
features. Axum validates a dedicated optional MapOrigin at startup and exposes
it through authenticated `GET /api/map`. The route owns this immutable state
without adding database fields. Dashboard server functions reuse the existing
private OpenHome transport and return only the parsed origin contract.

Both private coordinates must be supplied together. Missing configuration has
no geographic default. An explicit `MAP_DEMO_ORIGIN=true` uses an unrelated
central Copenhagen point and cannot be mixed with private coordinates. Neither
configuration errors nor logs include the submitted values. Dashboard responses
are `no-store`; coordinates reach the browser only to draw the local map within
the existing private household dashboard boundary.

MapLibre GL JS presents a flat, north-up map using OpenFreeMap's Fiord style.
It loads after the view mounts, bundles its worker explicitly through Vite,
and releases its map on navigation, hidden tabs and graphics failure. A loading
timeout cannot resurrect a map when a delayed library import completes.
Zoom/recenter controls respect reduced motion. Tile and WebGL failures expose
retry states without blocking the rest of the dashboard. The map has no routing
or transit rules; those remain with their forthcoming Axum integrations.

The local 3D view tilts the same map and extrudes OpenFreeMap's OpenMapTiles
building heights from zoom 14, below the labels and overlays. Switching never
recreates the map, so overlays and the viewport survive. Pitch stays fixed and
north-up; gestures cannot tilt or rotate it. No globe, terrain or satellite
source is used. A style without the building source disables 3D with an
explanation. Losing the graphics context in 3D restarts the map in 2D at the
same viewport rather than failing the whole view.

The building layer is hidden in 2D rather than kept under a zero opacity, so
2D never pays for extrusions. MapLibre treats visibility as a layout change
and re-parses the visible tiles on each switch; that one-off cost per switch
was preferred over continuous extrusion work. The 3D coverage notice samples
only the central quarter of the view to keep each settle cheap.

OpenFreeMap's [public service](https://openfreemap.org/) and
[terms](https://openfreemap.org/tos/), checked 2026-10-11, permit free map use
without a key, registration or paid plan. Its built-in OpenMapTiles and
OpenStreetMap attribution remains visible. Tiles load directly from the public
provider; its requests describe the displayed area. Availability is not
guaranteed, so the provider fallback remains necessary.

Browsers without WebGL2 cannot run MapLibre at all; the photo frame's Mali-400
WebView offers only WebGL1. The shared map probes for WebGL2 before importing
MapLibre and otherwise draws a flat Leaflet map of OpenStreetMap image tiles
with the same origin, routes and zoom/recenter controls, but no 3D. Image tiles
need no GPU parsing, which also suits the frame's 512 MB of memory. MapLibre 4
(WebGL1) was rejected as a second bundled major that still parses vector tiles;
CARTO's matching Positron raster now requires an API key. OpenStreetMap's
[tile policy](https://operations.osmfoundation.org/policies/tiles/), checked
2026-10-16, permits keyless light use with visible attribution and the browser's
Referer intact; the dashboard sets no restrictive Referrer-Policy. Night mode
darkens the light tiles with one CSS filter on the tile pane.

Each attempt to bring up either map has one owner for its pending import, load
timeout and map, advancing importing, starting, running and released only
forwards. Timeout, provider failure, graphics failure, a hidden tab, retry,
unmount and the 3D-to-2D restart all release through it, which stops the timer
and removes the MapLibre or Leaflet map with its own listeners and overlays.
An import, style, tile or graphics event arriving after release does nothing,
so a late callback cannot revive a view that already failed or was replaced.
The 2D restart now removes the lost map immediately rather than on the next
render. The view's status, the 2D/3D perspective and the building coverage stay
separate facts: each is already a small explicit union written by one path, and
merging them with the resource would couple independent policy. Rotation and
pitch gestures stay disabled through the libraries' own options and handlers
rather than being reimplemented. The vector map stops its timeout when the style
loads, before the overlay layers are added. MapLibre 6 calls `load` listeners
without catching, so an exception there would leave the view loading with no
timer; it is unproved rather than guaranteed away. Invalid layers are reported
as `error` events, and the only throwing call, a duplicate source id, cannot
occur on a fresh style. No test can reach `load` without WebGL, and moving the
timeout stop to the end would turn an overlay `error` event during setup into a
provider failure, so the order is kept. Real WebGL, tile provider and touch
behavior cannot be exercised in the test environment and need browser or device
acceptance.
