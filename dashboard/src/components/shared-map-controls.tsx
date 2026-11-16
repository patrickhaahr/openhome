import { CrosshairSimpleIcon } from "@phosphor-icons/react/dist/csr/CrosshairSimple"
import { MinusIcon } from "@phosphor-icons/react/dist/csr/Minus"
import { PlusIcon } from "@phosphor-icons/react/dist/csr/Plus"
import { Button } from "@/components/ui/button"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import type { Perspective } from "@/components/shared-map-config"

/** The zoom, perspective and recentre plate floating on the map. */
export function MapControls({
  perspective,
  buildingsUnavailable,
  choose,
  zoom,
  recenter,
}: {
  readonly perspective: Perspective
  readonly buildingsUnavailable: boolean
  readonly choose: (next: Perspective) => void
  readonly zoom: (by: 1 | -1) => void
  readonly recenter: () => void
}): React.JSX.Element {
  return (
    <div className="shared-map-controls">
      <div className="shared-map-perspective">
        <ToggleGroup
          aria-label="Map perspective"
          orientation="vertical"
          spacing={0}
          value={[perspective.mode]}
          onValueChange={([next]) => {
            if (next === "2d") choose({ mode: "2d", reason: "chosen" })

            if (next === "3d") choose({ mode: "3d" })
          }}
        >
          <ToggleGroupItem value="2d" size="lg">
            2D
          </ToggleGroupItem>
          <ToggleGroupItem
            value="3d"
            size="lg"
            aria-label="3D buildings"
            disabled={buildingsUnavailable}
          >
            3D
          </ToggleGroupItem>
        </ToggleGroup>
      </div>
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
  )
}
