import { useState } from "react"
import { SunDimIcon } from "@phosphor-icons/react/dist/csr/SunDim"

type FaderProps = {
  label: string
  /** 0 to 100. */
  value: number
  orientation: "vertical" | "horizontal"
  /** The light the fill is painted in. */
  tone: string
  disabled?: boolean
  /** The controller's readable level, drawn as a quiet notch; null when nothing was read. */
  readback?: number | null
  onChange: (value: number) => void
  /** The hand lifted: a good moment to send. */
  onRelease?: () => void
}

const keySteps = new Map([
  ["ArrowUp", (v: number) => v + 1],
  ["ArrowRight", (v: number) => v + 1],
  ["ArrowDown", (v: number) => v - 1],
  ["ArrowLeft", (v: number) => v - 1],
  ["PageUp", (v: number) => v + 10],
  ["PageDown", (v: number) => v - 10],
  ["Home", () => 0],
  ["End", () => 100],
])

/**
 * A capsule filled with the light it sets, like a dimmer under a thumb: touch anywhere and the
 * fill follows the finger. The figure only shows while the hand is on it.
 */
export function LightFader({
  label,
  value,
  orientation,
  tone,
  disabled = false,
  readback,
  onChange,
  onRelease,
}: FaderProps): React.JSX.Element {
  const [held, setHeld] = useState(false)

  function follow(event: React.PointerEvent<HTMLDivElement>): void {
    const box = event.currentTarget.getBoundingClientRect()

    const share =
      orientation === "vertical"
        ? (box.bottom - event.clientY) / box.height
        : (event.clientX - box.left) / box.width

    if (!Number.isFinite(share)) return

    const next = Math.round(Math.min(1, Math.max(0, share)) * 100)

    if (next !== value) onChange(next)
  }

  return (
    <div
      className="fader"
      role="slider"
      tabIndex={disabled ? -1 : 0}
      aria-label={label}
      aria-orientation={orientation}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={value}
      aria-valuetext={`${value}%`}
      aria-disabled={disabled}
      data-orientation={orientation}
      data-held={held}
      // SAFETY: CSSProperties omits custom properties; --fill is 0..1, --tone a CSS colour.
      style={
        {
          "--fill": value / 100,
          "--tone": tone,
        } as React.CSSProperties
      }
      onPointerDown={(event) => {
        if (disabled) return
        event.currentTarget.setPointerCapture(event.pointerId)
        setHeld(true)
        follow(event)
      }}
      onPointerMove={(event) => {
        if (held) follow(event)
      }}
      onPointerUp={() => {
        if (!held) return
        setHeld(false)
        onRelease?.()
      }}
      onPointerCancel={() => {
        setHeld(false)
        onRelease?.()
      }}
      onKeyDown={(event) => {
        const step = keySteps.get(event.key)

        if (disabled || step === undefined) return
        event.preventDefault()

        const next = Math.min(100, Math.max(0, step(value)))

        if (next !== value) onChange(next)
      }}
    >
      <span className="fader-fill" aria-hidden="true" />
      {readback === null || readback === undefined ? null : (
        <span
          className="fader-readback"
          aria-hidden="true"
          // SAFETY: CSSProperties omits custom properties; --at is the 0..1 read level.
          style={{ "--at": readback / 100 } as React.CSSProperties}
        />
      )}
      <span className="fader-figure num" aria-hidden="true">
        {value}
      </span>
      <SunDimIcon
        className="fader-glyph"
        size={22}
        weight="bold"
        aria-hidden="true"
      />
    </div>
  )
}
