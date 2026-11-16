import { CrosshairSimpleIcon } from "@phosphor-icons/react/dist/csr/CrosshairSimple"
import { MinusIcon } from "@phosphor-icons/react/dist/csr/Minus"
import { PlusIcon } from "@phosphor-icons/react/dist/csr/Plus"
import { Button } from "@/components/ui/button"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import type { Perspective } from "@/components/shared-map-config"

/**
 * The zoom, perspective and recentre plate floating on the map. The image-tile map has no
 * buildings, so it goes without the perspective toggle.
 */
export function MapControls({
  perspective,
  zoom,
  recenter,
}: {
  readonly perspective?: {
    readonly current: Perspective
    readonly buildingsUnavailable: boolean
    readonly choose: (next: Perspective) => void
  }
  readonly zoom: (by: 1 | -1) => void
  readonly recenter: () => void
}): React.JSX.Element {
  return (
    // One tall plate on the right edge, centred like Home's deck.
    <div className="absolute top-1/2 right-travel-inset z-2 grid -translate-y-1/2 justify-items-center gap-2 rounded-travel-plate bg-ground p-2 max-tablet:top-travel-inset max-tablet:translate-none">
      {perspective === undefined ? null : (
        <ToggleGroup
          aria-label="Map perspective"
          orientation="vertical"
          variant="plate"
          size="plate"
          value={[perspective.current.mode]}
          onValueChange={([next]) => {
            if (next === "2d")
              perspective.choose({ mode: "2d", reason: "chosen" })

            if (next === "3d") perspective.choose({ mode: "3d" })
          }}
        >
          <ToggleGroupItem value="2d">2D</ToggleGroupItem>
          <ToggleGroupItem
            value="3d"
            aria-label="3D buildings"
            disabled={perspective.buildingsUnavailable}
          >
            3D
          </ToggleGroupItem>
        </ToggleGroup>
      )}
      <Button
        size="map-key"
        variant="tone"
        aria-label="Zoom in"
        onClick={() => zoom(1)}
      >
        <PlusIcon size={22} aria-hidden="true" />
      </Button>
      <Button
        size="map-key"
        variant="tone"
        aria-label="Zoom out"
        onClick={() => zoom(-1)}
      >
        <MinusIcon size={22} aria-hidden="true" />
      </Button>
      <Button
        size="map-key"
        variant="tone"
        aria-label="Return to origin"
        onClick={recenter}
      >
        <CrosshairSimpleIcon size={22} aria-hidden="true" />
      </Button>
    </div>
  )
}
