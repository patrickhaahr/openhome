import { useEffect, useRef, useState } from "react";
import { Map, LngLatBounds, type Map as MapLibreMap } from "maplibre-gl";
import { DEMO_CENTER, DEMO_ROUTE } from "./demo-route";
import "./map-controls.css";

type Props = {
  readonly mode: "2d" | "3d";
  readonly showRoute?: boolean;
  readonly overview?: boolean;
};

/** An actual public-data map; the route itself is explicitly illustrative. */
export function DemoMap({ mode, showRoute = true, overview = false }: Props): React.JSX.Element {
  const container = useRef<HTMLDivElement>(null);
  const map = useRef<MapLibreMap | null>(null);
  const [state, setState] = useState<"loading" | "ready" | "unavailable">("loading");

  useEffect(() => {
    let active = true;
    let instance: MapLibreMap | undefined;
    Promise.resolve()
      .then(() => {
        if (!active || container.current === null) return;
        instance = new Map({
          container: container.current,
          style: "https://tiles.openfreemap.org/styles/dark",
          center: DEMO_CENTER,
          zoom: 14.35,
          attributionControl: overview ? false : { compact: true },
        });
        map.current = instance;
        instance.on("load", () => {
          if (!active || instance === undefined || container.current === null) return;
          if (showRoute) {
            const routeColor = getComputedStyle(container.current)
              .getPropertyValue("--color-graphic-route")
              .trim();
            instance.addSource("demo-route", { type: "geojson", data: DEMO_ROUTE });
            instance.addLayer({
              id: "route-glow",
              type: "line",
              source: "demo-route",
              paint: { "line-color": routeColor, "line-width": 10, "line-opacity": 0.12 },
            });
            instance.addLayer({
              id: "route",
              type: "line",
              source: "demo-route",
              layout: { "line-join": "round", "line-cap": "round" },
              paint: { "line-color": routeColor, "line-width": 3 },
            });
          }
          if (instance.getSource("openmaptiles") !== undefined) {
            instance.addLayer({
              id: "demo-buildings",
              type: "fill-extrusion",
              source: "openmaptiles",
              "source-layer": "building",
              minzoom: 13,
              filter: ["!=", ["get", "hide_3d"], true],
              paint: {
                "fill-extrusion-color": "#626262",
                "fill-extrusion-height": ["coalesce", ["get", "render_height"], 8],
                "fill-extrusion-base": ["coalesce", ["get", "render_min_height"], 0],
                "fill-extrusion-opacity": 0.75,
              },
            });
          }
          if (overview && showRoute) {
            const bounds = new LngLatBounds();
            for (const [longitude, latitude] of DEMO_ROUTE.geometry.coordinates) {
              if (longitude !== undefined && latitude !== undefined)
                bounds.extend([longitude, latitude]);
            }
            const fitRoute = () => {
              instance?.fitBounds(bounds, {
                padding: 12,
                duration: 0,
              });
            };
            fitRoute();
            instance.on("resize", fitRoute);
          }
          setState("ready");
        });
        instance.on("error", () => {
          if (active) setState("unavailable");
        });
      })
      .catch(() => {
        if (active) setState("unavailable");
      });
    return () => {
      active = false;
      instance?.remove();
      map.current = null;
    };
  }, [showRoute, overview]);

  useEffect(() => {
    if (state !== "ready") return;
    const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
    map.current?.easeTo({
      pitch: mode === "3d" ? 58 : 0,
      bearing: mode === "3d" ? -22 : 0,
      duration: reduced ? 0 : 220,
    });
  }, [mode, state]);

  return (
    <>
      <div className="map-shell" data-map-state={state} data-map-mode={mode}>
        <div
          ref={container}
          className="map-canvas"
          role="img"
          aria-label={`Public Copenhagen demo map in ${mode.toUpperCase()}, ${showRoute ? "with an illustrative walking loop" : "without live vehicle positions"}`}
        />
        {overview ? null : (
          <span className="map-place">
            <i /> Copenhagen · demo area
          </span>
        )}
        {state === "loading" ? (
          <span className="map-message" role="status">
            Loading public map…
          </span>
        ) : null}
        {state === "unavailable" ? (
          <span className="map-message" role="status">
            Map unavailable. The other panels still work.
          </span>
        ) : null}
        {overview ? null : (
          <span className="map-note">
            {showRoute ? "Illustrative loop" : "Transit map preview"} · OpenFreeMap / OSM
          </span>
        )}
      </div>
      {overview ? (
        <small className="map-credits">
          <a href="https://openfreemap.org/" target="_blank" rel="noreferrer">
            OpenFreeMap
          </a>
          <span> · </span>
          <a href="https://openmaptiles.org/" target="_blank" rel="noreferrer">
            © OpenMapTiles
          </a>
          <span> · </span>
          <a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noreferrer">
            © OpenStreetMap
          </a>
        </small>
      ) : null}
    </>
  );
}
