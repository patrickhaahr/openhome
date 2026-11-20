import { Button } from "@/components/ui/button"
import { chipFor } from "@/components/backlight-look"
import { rgbCss } from "@/lib/backlight-light"
import type { BacklightAction, BacklightSelection } from "@/lib/backlight"

export type DockKey = {
  id: string
  label: string
  detail?: string
  selected: boolean
  disabled: boolean
  selection: BacklightSelection
}

export function BacklightDock({
  keys,
  rgb,
  outcome,
  perform,
}: {
  keys: DockKey[]
  rgb: { red: number; green: number; blue: number }
  outcome: "pending" | "failed" | "accepted" | undefined
  perform: (action: BacklightAction) => void
}): React.JSX.Element {
  return (
    <div
      className="absolute bottom-flow left-1/2 z-2 flex max-w-led-dock -translate-x-1/2 items-center gap-1.5 rounded-full bg-led-wash p-2 **:aria-pressed:-translate-y-1.5 **:aria-pressed:bg-transparent **:aria-pressed:inset-ring-2 **:aria-pressed:inset-ring-led-ink **:data-[slot=button]:size-15 **:data-[slot=button]:p-0 **:data-[slot=button]:text-led-ink **:data-[slot=button]:transition-dock **:data-[slot=button]:duration-dock **:data-[slot=button]:ease-app **:data-[slot=button]:hover:bg-led-wash-strong **:data-[slot=button]:hover:bg-clip-border led-phone:w-led-dock led-phone:flex-wrap led-phone:justify-center led-phone:rounded-panel led-phone:**:data-[slot=button]:size-12"
      role="group"
      aria-label="Backlight selection"
    >
      {keys.map((key, index) => {
        const chip = chipFor(key.id)

        return (
          <Button
            key={key.id}
            variant="ghost"
            className={
              keys[index - 1]?.id === "color" ? "ml-3.5 led-phone:ml-0" : ""
            }
            disabled={key.disabled}
            aria-pressed={key.selected}
            data-outcome={key.selected ? outcome : undefined}
            title={key.label}
            // SAFETY: CSSProperties omits custom properties; --swatch is a CSS colour.
            style={
              key.id === "color"
                ? ({ "--swatch": rgbCss(rgb) } as React.CSSProperties)
                : undefined
            }
            onClick={() =>
              perform({ action: "select", selection: key.selection })
            }
          >
            {key.id === "color" ? (
              <span
                className="absolute inset-1.25 rounded-full bg-led-hue-ring"
                aria-hidden="true"
              />
            ) : null}
            <span
              className={`absolute rounded-full ${chip.fill} ${key.selected ? chip.motion : ""}`}
              aria-hidden="true"
            />
            <span className="sr-only">{key.label}</span>
            {key.detail === undefined ? null : (
              <span className="sr-only">{key.detail}</span>
            )}
          </Button>
        )
      })}
    </div>
  )
}
