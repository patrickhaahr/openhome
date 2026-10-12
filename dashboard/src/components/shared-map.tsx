import { useEffect, useRef, useState } from "react"
import type { Map as MapLibreMap } from "maplibre-gl"
import workerUrl from "maplibre-gl/dist/maplibre-gl-worker.mjs?worker&url"
import { CrosshairSimpleIcon } from "@phosphor-icons/react/dist/csr/CrosshairSimple"
import { MinusIcon } from "@phosphor-icons/react/dist/csr/Minus"
import { PlusIcon } from "@phosphor-icons/react/dist/csr/Plus"
import { Button } from "@/components/ui/button"
import type { MapOrigin } from "@/lib/map"

/** OpenFreeMap's free Fiord style; it needs no key and carries its own attribution. */
const STYLE_URL = "https://tiles.openfreemap.org/styles/fiord"

const LOCAL_ZOOM = 14

/** A stalled style or tile request must not leave the view loading forever. */
const LOAD_TIMEOUT_MS = 20_000

/** The dark theme's primary accent; MapLibre cannot read CSS oklch tokens. */
const ORIGIN_COLOR = "#ecca6c"

type MapView =
  | { state: "loading" }
  | { state: "ready"; tiles: "complete" | "incomplete" }
  | { state: "provider-unavailable" | "graphics-unavailable" | "failed" }

function prefersReducedMotion(): boolean {
  return matchMedia("(prefers-reduced-motion: reduce)").matches
}

/**
 * Interactive 2D map centered on the private origin. MapLibre loads only after
 * mount and is removed on unmount, releasing its WebGL context when the view closes.
 */
export function SharedMap({
  origin,
}: {
  origin: MapOrigin
}): React.JSX.Element {
  const container = useRef<HTMLDivElement>(null)
  const map = useRef<MapLibreMap | undefined>(undefined)
  const [view, setView] = useState<MapView>({ state: "loading" })
  const [attempt, setAttempt] = useState(0)
  const [visible, setVisible] = useState(true)
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

    setView({ state: "loading" })

    let instance: MapLibreMap | undefined
    let disposed = false

    function fail(state: Exclude<MapView["state"], "loading" | "ready">): void {
      if (disposed) return

      // Cancel the pending import as well as the map: a late import must not
      // resurrect a view that already timed out or lost its graphics context.
      disposed = true
      clearTimeout(timeout)
      const failed = instance

      instance = undefined
      map.current = undefined
      failed?.remove()
      setView({ state })
    }

    const timeout = setTimeout(() => {
      fail("provider-unavailable")
    }, LOAD_TIMEOUT_MS)

    import("maplibre-gl").then(
      ({ Map, setWorkerUrl }) => {
        if (disposed) return

        try {
          // Vite moves the library into a chunk; its relative worker URL then
          // points at a missing file. Bundle the worker explicitly for both builds.
          setWorkerUrl(workerUrl)
          instance = new Map({
            container: element,
            style: STYLE_URL,
            center: [origin.longitude, origin.latitude],
            zoom: LOCAL_ZOOM,
            minZoom: 10,
            maxZoom: 18,
            maxPitch: 0,
            dragRotate: false,
            pitchWithRotate: false,
            touchPitch: false,
            attributionControl: { compact: false },
            fadeDuration: prefersReducedMotion() ? 0 : 300,
          })
        } catch {
          // MapLibre throws when WebGL2 cannot be created.
          fail("graphics-unavailable")

          return
        }

        const created = instance
        let loaded = false

        map.current = created
        created.touchZoomRotate.disableRotation()
        created.keyboard.disableRotation()

        created.on("load", () => {
          loaded = true
          clearTimeout(timeout)
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
              "circle-color": ORIGIN_COLOR,
              "circle-opacity": 0.18,
            },
          })
          created.addLayer({
            id: "origin",
            type: "circle",
            source: "origin",
            paint: {
              "circle-radius": 7,
              "circle-color": ORIGIN_COLOR,
              "circle-stroke-width": 3,
              "circle-stroke-color": "#141414",
            },
          })

          if (!disposed) setView({ state: "ready", tiles: "complete" })
        })
        created.on("error", () => {
          // Before load the style or source failed; afterwards only some tiles did.
          if (!loaded) {
            fail("provider-unavailable")

            return
          }

          if (!disposed) setView({ state: "ready", tiles: "incomplete" })
        })
        created.on("webglcontextlost", () => {
          fail("graphics-unavailable")
        })
      },
      () => {
        fail("failed")
      }
    )

    return () => {
      disposed = true
      clearTimeout(timeout)
      instance?.remove()
      map.current = undefined
    }
  }, [origin.latitude, origin.longitude, attempt, visible])

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
      {view.state === "ready" ? (
        <div className="shared-map-controls">
          <Button
            size="icon-lg"
            variant="secondary"
            aria-label="Zoom in"
            onClick={() => zoom(1)}
          >
            <PlusIcon size={22} aria-hidden="true" />
          </Button>
          <Button
            size="icon-lg"
            variant="secondary"
            aria-label="Zoom out"
            onClick={() => zoom(-1)}
          >
            <MinusIcon size={22} aria-hidden="true" />
          </Button>
          <Button
            size="icon-lg"
            variant="secondary"
            aria-label="Return to origin"
            onClick={recenter}
          >
            <CrosshairSimpleIcon size={22} aria-hidden="true" />
          </Button>
        </div>
      ) : null}
      <MapStatus view={view} reload={reload} />
    </div>
  )
}

function MapStatus({
  view,
  reload,
}: {
  view: MapView
  reload: () => void
}): React.JSX.Element | null {
  switch (view.state) {
    case "loading":
      return (
        <p className="shared-map-message" role="status">
          Loading map…
        </p>
      )
    case "ready":
      return view.tiles === "incomplete" ? (
        <div className="shared-map-notice" role="status">
          <p>Some map tiles could not load. The map may be incomplete.</p>
          <Button variant="outline" onClick={reload}>
            Reload map
          </Button>
        </div>
      ) : null
    case "provider-unavailable":
      return (
        <MapFailure
          title="Map tiles unavailable"
          detail="OpenFreeMap could not be reached. The rest of the dashboard still works."
          reload={reload}
        />
      )
    case "graphics-unavailable":
      return (
        <MapFailure
          title="Map graphics unavailable"
          detail="This browser could not start WebGL2 map rendering. The rest of the dashboard still works."
          reload={reload}
        />
      )
    case "failed":
      return (
        <MapFailure
          title="Map unavailable"
          detail="The map could not start. Check the dashboard connection and try again."
          reload={reload}
        />
      )
  }
}

function MapFailure({
  title,
  detail,
  reload,
}: {
  title: string
  detail: string
  reload: () => void
}): React.JSX.Element {
  return (
    <div className="shared-map-message" role="status">
      <h3>{title}</h3>
      <p>{detail}</p>
      <Button size="lg" variant="outline" onClick={reload}>
        Try again
      </Button>
    </div>
  )
}
