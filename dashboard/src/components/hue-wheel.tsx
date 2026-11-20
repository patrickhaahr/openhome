import { useState } from "react"
import { hsvToRgb, rgbCss, rgbToHsv } from "@/lib/backlight-light"

type Rgb = { red: number; green: number; blue: number }

type HueWheelProps = {
  rgb: Rgb
  active: boolean
  disabled: boolean
  onChange: (rgb: Rgb) => void
}

const hueKeys = new Map([
  ["ArrowRight", { hue: 5, saturation: 0 }],
  ["ArrowLeft", { hue: -5, saturation: 0 }],
  ["PageUp", { hue: 30, saturation: 0 }],
  ["PageDown", { hue: -30, saturation: 0 }],
  ["ArrowUp", { hue: 0, saturation: 0.05 }],
  ["ArrowDown", { hue: 0, saturation: -0.05 }],
])

export function HueWheel({
  rgb,
  active,
  disabled,
  onChange,
}: HueWheelProps): React.JSX.Element {
  const [held, setHeld] = useState(false)
  const { hue, saturation, value } = rgbToHsv(rgb)
  const radians = (hue * Math.PI) / 180

  function pick(event: React.PointerEvent<HTMLDivElement>): void {
    const box = event.currentTarget.getBoundingClientRect()
    const radius = box.width / 2

    if (radius <= 0) return

    const dx = event.clientX - (box.left + radius)
    const dy = event.clientY - (box.top + radius)
    const degrees = (Math.atan2(dx, -dy) * 180) / Math.PI

    onChange(
      hsvToRgb({
        hue: (degrees + 360) % 360,
        saturation: Math.min(1, Math.hypot(dx, dy) / radius),
        value: 1,
      })
    )
  }

  return (
    <div
      className="group/wheel relative aspect-square w-led-wheel cursor-crosshair touch-none rounded-full bg-led-hue-wheel transition-fade-scale duration-mid ease-app focus-visible:outline-offset-4 aria-disabled:cursor-default aria-disabled:opacity-20 data-[active=false]:scale-92 led-stacked:w-led-disc-stacked"
      role="slider"
      tabIndex={disabled ? -1 : 0}
      aria-label="RGB color"
      aria-valuemin={0}
      aria-valuemax={359}
      aria-valuenow={Math.round(hue)}
      aria-valuetext={`${rgb.red}, ${rgb.green}, ${rgb.blue}`}
      aria-disabled={disabled}
      data-active={active}
      data-held={held}
      // SAFETY: CSSProperties omits custom properties; the knob's position and colour.
      style={
        {
          "--knob-x": `${50 + 50 * saturation * Math.sin(radians)}%`,
          "--knob-y": `${50 - 50 * saturation * Math.cos(radians)}%`,
          "--knob": rgbCss(rgb),
        } as React.CSSProperties
      }
      onPointerDown={(event) => {
        if (disabled) return
        event.currentTarget.setPointerCapture(event.pointerId)
        setHeld(true)
        pick(event)
      }}
      onPointerMove={(event) => {
        if (held) pick(event)
      }}
      onPointerUp={() => setHeld(false)}
      onPointerCancel={() => setHeld(false)}
      onKeyDown={(event) => {
        const step = hueKeys.get(event.key)

        if (disabled || step === undefined) return
        event.preventDefault()

        const { hue: hueStep, saturation: saturationStep } = step
        const grid = Math.abs(hueStep) || 5

        onChange(
          hsvToRgb({
            hue: Math.round(hue / grid) * grid + hueStep,
            saturation: Math.min(
              1,
              Math.max(0, Math.round(saturation * 20) / 20 + saturationStep)
            ),
            value: value === 0 ? 1 : value,
          })
        )
      }}
    >
      <span
        className="pointer-events-none absolute top-(--knob-y) left-(--knob-x) size-11 -translate-1/2 rounded-full border-4 border-white bg-(--knob) transition-knob duration-knob group-data-[active=false]/wheel:opacity-0 group-data-[held=true]/wheel:size-14"
        aria-hidden="true"
      />
    </div>
  )
}
