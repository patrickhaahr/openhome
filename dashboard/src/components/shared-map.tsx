import { useEffect, useRef, useState } from "react"
import type { GeoJSONSource, Map as MapLibreMap } from "maplibre-gl"
import workerUrl from "maplibre-gl/dist/maplibre-gl-worker.mjs?worker&url"
import { RasterMap, startMapAttempt } from "@/components/raster-map"
import {
  evenPadding,
  routePadding,
  type MapLine,
  type MapOrigin,
  type MapRoutes,
} from "@/lib/map"
import {
  buildingsLayer,
  mapThemes,
  type MapTheme,
  LOCAL_ZOOM,
  BUILDINGS_PITCH,
  BUILDINGS_LAYER,
  ROUTE_SOURCE,
  BUILDINGS_SOURCE,
  type MapView,
  type Perspective,
  type BuildingCoverage,
  presentation,
  supportsWebGL2,
  prefersReducedMotion,
  routeFeatures,
  routeBounds,
  buildingCoverage,
} from "@/components/shared-map-config"
import { MapControls } from "@/components/shared-map-controls"
import { MapStatus } from "@/components/shared-map-status"

/**
 * Interactive map centered on the private origin, switchable between flat 2D
 * and pitched 3D buildings. Switching keeps the map, its overlays and viewport.
 * Supplied routes are drawn above the buildings, the focused one highlighted,
 * and fitted into view once per set of alternatives.
 * MapLibre loads only after mount and is removed on unmount, releasing its
 * WebGL context when the view closes. Browsers without WebGL2 get the flat
 * image-tile `RasterMap` instead.
 */
export function SharedMap({
  origin,
  routes,
  theme = "night",
  credit,
  controls = true,
}: {
  origin: MapOrigin
  routes?: MapRoutes | undefined
  /** Recreates the map in the other palette when the dashboard turns. */
  theme?: MapTheme
  /** An extra credit merged into the map's one attribution. */
  credit?: string | undefined
  /** The zoom, perspective and recentre plate; a map that only shows a route can go without. */
  controls?: boolean
}): React.JSX.Element {
  const container = useRef<HTMLDivElement>(null)
  const map = useRef<MapLibreMap | undefined>(undefined)
  const [view, setView] = useState<MapView>({ state: "loading" })
  const [attempt, setAttempt] = useState(0)
  const [visible, setVisible] = useState(true)
  // Without WebGL2 the image-tile map stands in, and MapLibre is never loaded.
  const [renderer, setRenderer] = useState<"vector" | "raster">("vector")

  const [perspective, setPerspective] = useState<Perspective>({
    mode: "2d",
    reason: "chosen",
  })

  const [coverage, setCoverage] = useState<BuildingCoverage>("shown")
  // A retried map starts in the current perspective without recreating on toggles.
  const mode = useRef<Perspective["mode"]>("2d")

  // The viewport a graphics restart resumes from, consumed by the next map.
  const resume = useRef<{ center: [number, number]; zoom: number } | undefined>(
    undefined
  )

  // A recreated map draws the current routes without refitting the viewport to them.
  const shownRoutes = useRef<MapRoutes | undefined>(routes)
  const fittedLines = useRef<ReadonlyArray<MapLine> | undefined>(undefined)

  const center: [number, number] = [origin.longitude, origin.latitude]

  useEffect(() => {
    function visibilityChanged(): void {
      setVisible(!document.hidden)
    }

    visibilityChanged()
    document.addEventListener("visibilitychange", visibilityChanged)

    return () =>
      document.removeEventListener("visibilitychange", visibilityChanged)
  }, [])

  useEffect(() => {
    const element = container.current

    if (element === null || !visible || document.hidden) return

    if (!supportsWebGL2()) {
      // eslint-disable-next-line react/set-state-in-effect -- WebGL2 can only be probed in the browser after hydration; SSR has no canvas.
      setRenderer("raster")

      return
    }

    setView({ state: "loading" })
    // A recreated map measures its own buildings; never show the last map's notice.
    setCoverage("shown")

    // The one owner of this attempt's import, timeout and map. Releasing it also
    // cancels the pending import: a late import, style or graphics event must not
    // resurrect a view that already timed out, failed or was replaced.
    const owner = startMapAttempt<MapLibreMap>({
      onTimeout: () => fail("provider-unavailable"),
      onMap: (next) => {
        map.current = next
      },
    })

    function fail(state: Exclude<MapView["state"], "loading" | "ready">): void {
      if (owner.phase() === "released") return

      owner.release()
      setView({ state })
    }

    import("maplibre-gl").then(
      ({ Map, setWorkerUrl }) => {
        if (owner.phase() === "released") return

        let created: MapLibreMap

        try {
          // Vite moves the library into a chunk; its relative worker URL then
          // points at a missing file. Bundle the worker explicitly for both builds.
          setWorkerUrl(workerUrl)
          created = new Map({
            container: element,
            style: mapThemes[theme].style,
            center: resume.current?.center ?? [
              origin.longitude,
              origin.latitude,
            ],
            zoom: resume.current?.zoom ?? LOCAL_ZOOM,
            minZoom: 10,
            maxZoom: 18,
            pitch: presentation(mode.current).pitch,
            // Only the perspective toggle tilts the map; pitch gestures stay off.
            maxPitch: BUILDINGS_PITCH,
            dragRotate: false,
            pitchWithRotate: false,
            touchPitch: false,
            attributionControl: {
              compact: false,
              customAttribution: credit === undefined ? [] : [credit],
            },
            fadeDuration: prefersReducedMotion() ? 0 : 300,
          })
        } catch {
          // MapLibre throws when WebGL2 cannot be created.
          fail("graphics-unavailable")

          return
        }

        resume.current = undefined

        // A route framed on its own is fitted again on a map made for the other palette.
        if (shownRoutes.current?.framing === "even")
          fittedLines.current = undefined

        owner.adopt(created)
        created.touchZoomRotate.disableRotation()
        created.keyboard.disableRotation()

        created.on("load", () => {
          if (!owner.settle()) return

          // A style without the building source still serves a usable 2D map.
          const buildings =
            created.getSource(BUILDINGS_SOURCE) === undefined
              ? "unavailable"
              : "available"

          if (buildings === "available") {
            // Below the labels, so street names stay readable over rooftops.
            const firstLabel = created
              .getStyle()
              .layers.find((layer) => layer.type === "symbol")?.id

            created.addLayer(
              {
                ...buildingsLayer(theme),
                layout: { visibility: presentation(mode.current).buildings },
              },
              firstLabel
            )
          }

          // Below the origin, so the start stays visible where the loop returns.
          created.addSource(ROUTE_SOURCE, {
            type: "geojson",
            data: routeFeatures(shownRoutes.current),
          })
          // Every casing sits below every line, and the sort key keeps the
          // highlighted route on top where loops share streets.
          created.addLayer({
            id: "route-casing",
            type: "line",
            source: ROUTE_SOURCE,
            layout: {
              "line-join": "round",
              "line-cap": "round",
              "line-sort-key": ["case", ["get", "focused"], 1, 0],
            },
            paint: {
              "line-color": mapThemes[theme].casing,
              "line-width": ["case", ["get", "focused"], 9, 6],
            },
          })
          created.addLayer({
            id: "route",
            type: "line",
            source: ROUTE_SOURCE,
            layout: {
              "line-join": "round",
              "line-cap": "round",
              "line-sort-key": ["case", ["get", "focused"], 1, 0],
            },
            paint: {
              "line-color": [
                "case",
                ["get", "focused"],
                mapThemes[theme].origin,
                mapThemes[theme].alternative,
              ],
              "line-width": ["case", ["get", "focused"], 5, 3],
              "line-opacity": ["case", ["get", "focused"], 1, 0.75],
            },
          })

          const endpoint = shownRoutes.current?.endpoint

          if (endpoint !== undefined) {
            created.addSource("endpoint", {
              type: "geojson",
              data: {
                type: "Feature",
                properties: {},
                geometry: { type: "Point", coordinates: [...endpoint] },
              },
            })
            // A disc under the start's dot, so a loop reads as start and finish in one place.
            created.addLayer({
              id: "endpoint",
              type: "circle",
              source: "endpoint",
              paint: {
                "circle-radius": 12,
                "circle-color": mapThemes[theme].casing,
                "circle-stroke-width": 3,
                "circle-stroke-color": mapThemes[theme].origin,
              },
            })
          }

          created.addSource("origin", {
            type: "geojson",
            data: {
              type: "Feature",
              properties: {},
              geometry: {
                type: "Point",
                coordinates: [origin.longitude, origin.latitude],
              },
            },
          })
          created.addLayer({
            id: "origin-halo",
            type: "circle",
            source: "origin",
            paint: {
              "circle-radius": 18,
              "circle-color": mapThemes[theme].origin,
              "circle-opacity": 0.18,
            },
          })
          created.addLayer({
            id: "origin",
            type: "circle",
            source: "origin",
            paint: {
              "circle-radius": 7,
              "circle-color": mapThemes[theme].origin,
              "circle-stroke-width": 3,
              "circle-stroke-color": mapThemes[theme].casing,
            },
          })

          setView({ state: "ready", tiles: "complete", buildings })
        })
        created.on("idle", () => {
          if (owner.phase() !== "running" || mode.current !== "3d") return

          setCoverage(buildingCoverage(created))
        })
        created.on("error", () => {
          // Before load the style or source failed; afterwards only some tiles did.
          switch (owner.phase()) {
            case "starting":
              fail("provider-unavailable")

              return
            case "running":
              setView((current) =>
                current.state === "ready"
                  ? { ...current, tiles: "incomplete" }
                  : current
              )

              return
            case "importing":
            case "released":
              return
          }
        })
        created.on("webglcontextlost", () => {
          if (owner.phase() === "released") return

          if (mode.current !== "3d") {
            fail("graphics-unavailable")

            return
          }

          // Extrusions can exhaust older GPUs; restart flat rather than blank.
          resume.current = {
            center: created.getCenter().toArray(),
            zoom: created.getZoom(),
          }
          owner.release()
          choose({ mode: "2d", reason: "graphics-failure" })
          setAttempt((count) => count + 1)
        })
      },
      () => {
        fail("failed")
      }
    )

    return () => {
      owner.release()
    }
  }, [origin.latitude, origin.longitude, attempt, visible, theme, credit])

  const ready = view.state === "ready"

  useEffect(() => {
    const current = map.current

    if (!ready || current === undefined) return

    const { pitch, buildings } = presentation(perspective.mode)

    // Visibility is a layout change, so MapLibre re-parses the visible tiles on
    // each switch; a hidden layer keeps 2D from paying for extrusions (ADR 0007).
    if (
      current.getLayer(BUILDINGS_LAYER) !== undefined &&
      current.getLayoutProperty(BUILDINGS_LAYER, "visibility") !== buildings
    )
      current.setLayoutProperty(BUILDINGS_LAYER, "visibility", buildings)

    // A freshly loaded map already has this pitch; easing would cancel a pan.
    if (current.getPitch() !== pitch)
      current.easeTo({
        pitch,
        duration: prefersReducedMotion() ? 0 : 500,
      })
  }, [perspective.mode, ready])

  useEffect(() => {
    shownRoutes.current = routes
    const current = map.current

    if (!ready || current === undefined) return

    // MapLibre applies the data on its worker; the fit below does not depend on it.
    void current
      .getSource<GeoJSONSource>(ROUTE_SOURCE)
      ?.setData(routeFeatures(routes))

    // Highlighting another alternative keeps the viewport the household chose.
    if (
      routes === undefined ||
      routes.lines.length === 0 ||
      fittedLines.current === routes.lines
    )
      return

    fittedLines.current = routes.lines
    current.fitBounds(routeBounds(routes.lines), {
      padding:
        routes.framing === "even"
          ? evenPadding
          : routePadding(
              current.getContainer().clientWidth,
              current.getContainer().clientHeight
            ),
      maxZoom: 16,
      pitch: current.getPitch(),
      duration: prefersReducedMotion() ? 0 : 600,
    })
  }, [routes, ready])

  function choose(next: Perspective): void {
    mode.current = next.mode

    // Coverage is measured again once the tilted map settles.
    if (next.mode === "3d") setCoverage("shown")
    setPerspective(next)
  }

  function reload(): void {
    setView({ state: "loading" })
    setAttempt((count) => count + 1)
  }

  function zoom(by: 1 | -1): void {
    map.current?.easeTo({
      zoom: map.current.getZoom() + by,
      duration: prefersReducedMotion() ? 0 : 250,
    })
  }

  function recenter(): void {
    map.current?.easeTo({
      center,
      zoom: LOCAL_ZOOM,
      duration: prefersReducedMotion() ? 0 : 400,
    })
  }

  if (renderer === "raster")
    return (
      <RasterMap
        origin={origin}
        routes={routes}
        palette={mapThemes[theme]}
        shade={theme}
        credit={credit}
        controls={controls}
      />
    )

  return (
    <div className="shared-map" data-map-state={view.state}>
      <div
        ref={container}
        className="shared-map-canvas"
        aria-label={
          origin.source === "demo"
            ? "Map of the Copenhagen demo area"
            : "Map of the local area"
        }
      />
      {view.state === "ready" && controls ? (
        <MapControls
          perspective={perspective}
          buildingsUnavailable={view.buildings === "unavailable"}
          choose={choose}
          zoom={zoom}
          recenter={recenter}
        />
      ) : null}
      <MapStatus
        view={view}
        perspective={perspective}
        coverage={coverage}
        reload={reload}
      />
    </div>
  )
}
