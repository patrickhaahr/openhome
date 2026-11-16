import { useEffect, useRef, useState } from "react"
import { MoonIcon } from "@phosphor-icons/react/dist/csr/Moon"
import { MoonStarsIcon } from "@phosphor-icons/react/dist/csr/MoonStars"
import { SunHorizonIcon } from "@phosphor-icons/react/dist/csr/SunHorizon"
import { Button } from "@/components/ui/button"
import { useIdleBlock } from "@/components/idle-block"
import { LightFader } from "@/components/light-fader"
import {
  angleClock,
  clockAngle,
  clockArc,
  clockMinutes,
  copenhagenMinutes,
  polar,
  stepClock,
} from "@/lib/backlight-light"
import { bedtimeFields, type BacklightAction } from "@/lib/backlight"
import type { BacklightStatus } from "@/lib/backlight"

type Bedtime = BacklightStatus["bedtime"]

type BedtimeDialProps = {
  saved: Bedtime
  /** The display clock; the sun sits on the ring at this minute. */
  now: Date | undefined
  /** The switch needs a saved TV Sync profile; the window can be edited regardless. */
  canSwitch: boolean
  pending: boolean
  save: (action: BacklightAction) => void
}

const size = 240

const centre = size / 2

const ring = 94

const hours = Array.from({ length: 24 }, (_, hour) => hour)

/** Keys move an end of the window by a quarter or a whole hour. */
const clockKeys = new Map([
  ["ArrowUp", 15],
  ["ArrowRight", 15],
  ["ArrowDown", -15],
  ["ArrowLeft", -15],
  ["PageUp", 60],
  ["PageDown", -60],
])

type End = "start" | "end"

/**
 * The night drawn as a day: a 24-hour ring with midnight at the top, the bedtime window as a
 * red arc between a moon and a sun, and the sun dot at this minute. Drag either end; the moon
 * in the middle turns the routine's red on and off. Edits send themselves when the hand lifts.
 */
export function BedtimeDial({
  saved,
  now,
  canSwitch,
  pending,
  save,
}: BedtimeDialProps): React.JSX.Element {
  const [draft, setDraft] = useState<Bedtime | null>(null)
  const [dragging, setDragging] = useState<End | null>(null)
  const settle = useRef<ReturnType<typeof setTimeout> | null>(null)
  const shown = draft ?? saved
  const editable = !pending

  useIdleBlock(draft !== null)

  useEffect(
    () => () => {
      if (settle.current !== null) clearTimeout(settle.current)
    },
    []
  )

  function commit(next: Bedtime | null = draft): void {
    if (settle.current !== null) clearTimeout(settle.current)
    settle.current = null

    if (next === null) return

    const changed: Extract<BacklightAction, { action: "bedtime" }> = {
      action: "bedtime",
    }

    if (next.start !== saved.start || next.end !== saved.end) {
      changed.start = next.start
      changed.end = next.end
    }

    if (next.brightness !== saved.brightness)
      changed.brightness = next.brightness

    if (Object.keys(changed).length === 1) {
      setDraft(null)

      return
    }

    if (!bedtimeFields.safeParse(next).success) {
      setDraft(null)

      return
    }

    save(changed)
  }

  function change(next: Bedtime, wait: boolean): void {
    setDraft(next)

    if (settle.current !== null) clearTimeout(settle.current)
    settle.current = wait ? setTimeout(() => commit(next), 700) : null
  }

  function point(event: React.PointerEvent<SVGSVGElement>, end: End): void {
    const box = event.currentTarget.getBoundingClientRect()

    if (box.width <= 0) return

    const dx = event.clientX - (box.left + box.width / 2)
    const dy = event.clientY - (box.top + box.height / 2)
    const time = angleClock((Math.atan2(dx, -dy) * 180) / Math.PI)
    const other = end === "start" ? shown.end : shown.start

    if (time === other || time === shown[end]) return
    change({ ...shown, [end]: time }, false)
  }

  const start = clockMinutes(shown.start)
  const end = clockMinutes(shown.end)
  const at = now === undefined ? null : copenhagenMinutes(now)

  return (
    <section
      className="grid w-led-dial justify-items-center gap-led-dial-gap led-stacked:w-led-disc-stacked"
      aria-label="Bedtime Mode"
    >
      <div className="relative aspect-square w-full">
        <svg
          viewBox={`0 0 ${size} ${size}`}
          className="size-full touch-none overflow-visible"
          onPointerMove={(event) => {
            if (dragging !== null) point(event, dragging)
          }}
          onPointerUp={() => {
            if (dragging === null) return
            setDragging(null)
            commit()
          }}
          onPointerCancel={() => {
            setDragging(null)
            commit()
          }}
        >
          <circle
            className="stroke-wash-strong"
            fill="none"
            strokeWidth={18}
            cx={centre}
            cy={centre}
            r={ring}
          />
          {hours.map((hour) => {
            const mark = polar(centre, ring - 22, clockAngle(hour * 60))

            return (
              <circle
                key={hour}
                className={
                  hour % 6 === 0 ? "fill-muted-ink" : "fill-line-strong"
                }
                cx={mark.x}
                cy={mark.y}
                r={hour % 6 === 0 ? 2.2 : 1.2}
              />
            )
          })}
          <path
            className={`transition-colors duration-mid ease-app ${saved.enabled ? "stroke-bedtime-red" : "stroke-line-strong"}`}
            fill="none"
            strokeWidth={18}
            strokeLinecap="round"
            d={clockArc(centre, ring, start, end)}
          />
          {at === null ? null : (
            <circle
              className="fill-sun"
              {...(({ x, y }) => ({ cx: x, cy: y }))(
                polar(centre, ring - 22, clockAngle(at))
              )}
              r={4.5}
            />
          )}
          {(["start", "end"] as const).map((which) => {
            const time = shown[which]
            const spot = polar(centre, ring, clockAngle(clockMinutes(time)))
            const other = which === "start" ? shown.end : shown.start
            const Glyph = which === "start" ? MoonIcon : SunHorizonIcon

            return (
              <g
                key={which}
                className="group/handle cursor-grab outline-none aria-disabled:cursor-default"
                role="slider"
                tabIndex={editable ? 0 : -1}
                aria-label={which === "start" ? "Start" : "End"}
                aria-valuemin={0}
                aria-valuemax={1439}
                aria-valuenow={clockMinutes(time)}
                aria-valuetext={time}
                aria-disabled={!editable}
                data-held={dragging === which}
                transform={`translate(${spot.x} ${spot.y})`}
                onPointerDown={(event) => {
                  if (!editable) return
                  event.currentTarget.ownerSVGElement?.setPointerCapture(
                    event.pointerId
                  )
                  setDragging(which)
                }}
                onKeyDown={(event) => {
                  const delta = clockKeys.get(event.key)

                  if (!editable || delta === undefined) return
                  event.preventDefault()
                  change(
                    { ...shown, [which]: stepClock(time, delta, other) },
                    true
                  )
                }}
              >
                <circle className="fill-transparent" r={24} />
                <circle
                  className={`fill-ground transition-r duration-fast ease-app group-focus-visible/handle:stroke-ink group-focus-visible/handle:stroke-4 group-data-[held=true]/handle:r-held ${saved.enabled ? "stroke-bedtime-red" : "stroke-line-strong"}`}
                  strokeWidth={3}
                  r={15}
                />
                <Glyph
                  x={-9}
                  y={-9}
                  size={18}
                  weight="fill"
                  aria-hidden="true"
                />
              </g>
            )
          })}
        </svg>
        <div className="pointer-events-none absolute inset-0 grid place-content-center justify-items-center gap-2 *:pointer-events-auto **:data-[slot=button]:data-[on=true]:bg-bedtime-red **:data-[slot=button]:data-[on=true]:bg-clip-border **:data-[slot=button]:data-[on=true]:text-white">
          <Button
            size="key-round"
            variant="outline"
            role="switch"
            aria-label="Bedtime Mode enabled"
            aria-checked={saved.enabled}
            data-on={saved.enabled}
            disabled={pending || !canSwitch}
            title="Bedtime Mode"
            onClick={() => save({ action: "bedtime", enabled: !saved.enabled })}
          >
            <MoonStarsIcon
              size={28}
              weight={saved.enabled ? "fill" : "regular"}
              aria-hidden="true"
            />
          </Button>
          <p className="text-base font-medium num text-muted-ink">
            {shown.start}–{shown.end}
          </p>
        </div>
      </div>
      <div className="h-11 w-41/50">
        <LightFader
          label="Red brightness"
          orientation="horizontal"
          tone="bg-bedtime-red"
          value={shown.brightness}
          disabled={!editable}
          onChange={(brightness) => change({ ...shown, brightness }, true)}
          onRelease={() => commit()}
        />
      </div>
    </section>
  )
}
