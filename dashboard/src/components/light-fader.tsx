import { useState } from "react"
import { SunDimIcon } from "@phosphor-icons/react/dist/csr/SunDim"

type FaderProps = {
  label: string
  value: number
  orientation: "vertical" | "horizontal"
  tone: string
  disabled?: boolean
  readback?: number | null
  onChange: (value: number) => void
  onRelease?: () => void
}

const looks = {
  vertical: {
    track: "rounded-sheet bg-led-wash-strong",
    figure:
      "inset-x-0 top-3 translate-y-2 text-led-figure group-focus-visible/fader:translate-y-0 group-data-[held=true]/fader:translate-y-0",
    glyph: "bottom-4 left-1/2 -translate-x-1/2",
  },
  horizontal: {
    track: "rounded-full bg-wash-strong",
    figure:
      "top-1/2 right-4 translate-x-2 -translate-y-1/2 text-body-lg group-focus-visible/fader:translate-x-0 group-data-[held=true]/fader:translate-x-0",
    glyph: "top-1/2 left-3.5 -translate-y-1/2",
  },
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

  const glyphTone =
    orientation === "vertical"
      ? "text-led-glyph"
      : value === 0
        ? "text-muted-ink"
        : "text-white"

  return (
    <div
      className={`group/fader relative size-full cursor-grab touch-none overflow-hidden transition-transform duration-fast ease-app aria-disabled:cursor-default aria-disabled:opacity-40 data-[held=true]:scale-103 data-[held=true]:cursor-grabbing ${looks[orientation].track}`}
      role="slider"
      tabIndex={disabled ? -1 : 0}
      aria-label={label}
      aria-orientation={orientation}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={value}
      aria-valuetext={`${value}%`}
      aria-disabled={disabled}
      data-held={held}
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
      <span
        className={`absolute inset-0 transition-transform duration-fast ease-app group-data-[held=true]/fader:transition-none ${tone} ${
          orientation === "vertical"
            ? "translate-y-(--shift)"
            : "translate-x-(--shift)"
        }`}
        aria-hidden="true"
        // SAFETY: CSSProperties omits custom properties; --shift is the unfilled share of the track.
        style={
          {
            "--shift": `${
              orientation === "vertical" ? 100 - value : value - 100
            }%`,
          } as React.CSSProperties
        }
      />
      {readback === null || readback === undefined ? null : (
        <span
          className="absolute inset-x-11/50 bottom-(--at) -mb-led-notch-half h-0.75 rounded-full bg-led-ink shadow-led-notch transition-bottom duration-mid ease-app"
          aria-hidden="true"
          // SAFETY: CSSProperties omits custom properties; --at is the read level as a share of the track.
          style={{ "--at": `${readback}%` } as React.CSSProperties}
        />
      )}
      <span
        className={`absolute text-center font-semibold num text-white opacity-0 mix-blend-difference transition-reveal duration-fast ease-app group-focus-visible/fader:opacity-100 group-data-[held=true]/fader:opacity-100 ${looks[orientation].figure}`}
        aria-hidden="true"
      >
        {value}
      </span>
      <SunDimIcon
        className={`absolute ${glyphTone} ${looks[orientation].glyph}`}
        size={22}
        weight="bold"
        aria-hidden="true"
      />
    </div>
  )
}
