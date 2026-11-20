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
import { MapFrame } from "@/components/shared-map-frame"
import { MapStatus } from "@/components/shared-map-status"

export function SharedMap({
  origin,
  routes,
  theme = "night",
  credit,
  controls = true,
}: {
  origin: MapOrigin
  routes?: MapRoutes | undefined
  theme?: MapTheme
  credit?: string | undefined
  controls?: boolean
}): React.JSX.Element {
  const container = useRef<HTMLDivElement>(null)
  const map = useRef<MapLibreMap | undefined>(undefined)
  const [view, setView] = useState<MapView>({ state: "loading" })
  const [attempt, setAttempt] = useState(0)
  const [visible, setVisible] = useState(true)
  const [renderer, setRenderer] = useState<"vector" | "raster">("vector")

  const [perspective, setPerspective] = useState<Perspective>({
    mode: "2d",
    reason: "chosen",
  })

  const [coverage, setCoverage] = useState<BuildingCoverage>("shown")
  const mode = useRef<Perspective["mode"]>("2d")

  const resume = useRef<{ center: [number, number]; zoom: number } | undefined>(
    undefined
  )

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
    setCoverage("shown")

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
          fail("graphics-unavailable")

          return
        }

        resume.current = undefined

        if (shownRoutes.current?.framing === "even")
          fittedLines.current = undefined

        owner.adopt(created)
        created.touchZoomRotate.disableRotation()
        created.keyboard.disableRotation()

        created.on("load", () => {
          if (!owner.settle()) return

          const buildings =
            created.getSource(BUILDINGS_SOURCE) === undefined
              ? "unavailable"
              : "available"

          if (buildings === "available") {
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

          created.addSource(ROUTE_SOURCE, {
            type: "geojson",
            data: routeFeatures(shownRoutes.current),
          })
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

    if (
      current.getLayer(BUILDINGS_LAYER) !== undefined &&
      current.getLayoutProperty(BUILDINGS_LAYER, "visibility") !== buildings
    )
      current.setLayoutProperty(BUILDINGS_LAYER, "visibility", buildings)

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

    void current
      .getSource<GeoJSONSource>(ROUTE_SOURCE)
      ?.setData(routeFeatures(routes))

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
    <MapFrame state={view.state} canvas={container} source={origin.source}>
      {view.state === "ready" && controls ? (
        <MapControls
          perspective={{
            current: perspective,
            buildingsUnavailable: view.buildings === "unavailable",
            choose,
          }}
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
    </MapFrame>
  )
}
