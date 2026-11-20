import type { MapOrigin } from "@/lib/map"

export function MapFrame({
  state,
  renderer,
  shade,
  canvas,
  source,
  children,
}: {
  readonly state: string
  readonly renderer?: "raster"
  readonly shade?: "day" | "night"
  readonly canvas: React.Ref<HTMLDivElement>
  readonly source: MapOrigin["source"]
  readonly children: React.ReactNode
}): React.JSX.Element {
  return (
    <div
      className="group/map relative map-chrome h-full overflow-hidden bg-wash-strong"
      data-map-state={state}
      data-map-renderer={renderer}
      data-map-shade={shade}
    >
      <div
        ref={canvas}
        className="invisible map-canvas group-data-[map-state=ready]/map:visible"
        aria-label={
          source === "demo"
            ? "Map of the Copenhagen demo area"
            : "Map of the local area"
        }
      />
      {children}
    </div>
  )
}
