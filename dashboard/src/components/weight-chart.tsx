import { useEffect, useRef, useState } from "react"
import type { BodyWeight } from "@/lib/body-profile"

const kilograms = new Intl.NumberFormat("en-GB", {
  minimumFractionDigits: 1,
  maximumFractionDigits: 1,
})

const shortDate = new Intl.DateTimeFormat("en-GB", {
  timeZone: "UTC",
  day: "numeric",
  month: "short",
})

const monthName = new Intl.DateTimeFormat("en-GB", {
  timeZone: "UTC",
  month: "short",
})

/** The width drawn before the figure is measured, and the narrowest drawn at all. */
const defaultWidth = 840

/** Room for the kilogram labels at the right edge. */
const pad = { top: 28, right: 64, bottom: 36, left: 24 }

const day = 86_400_000

/** Days either side of a weigh-in averaged into the trend line. */
const smoothingDays = 3

type Point = {
  readonly x: number
  readonly y: number
  readonly entry: BodyWeight
  readonly trend: number
}

function time(date: string): number {
  return Date.parse(`${date}T00:00:00Z`)
}

/** A smooth path through points (Catmull-Rom as cubic Béziers), so the trend reads as a trend. */
function smooth(points: ReadonlyArray<{ x: number; y: number }>): string {
  if (points.length === 0) return ""

  let path = `M${points[0]?.x ?? 0} ${points[0]?.y ?? 0}`

  for (let index = 0; index < points.length - 1; index++) {
    const p0 = points[index - 1] ?? points[index]
    const p1 = points[index]
    const p2 = points[index + 1]
    const p3 = points[index + 2] ?? p2

    if (
      p0 === undefined ||
      p1 === undefined ||
      p2 === undefined ||
      p3 === undefined
    )
      continue

    const c1 = { x: p1.x + (p2.x - p0.x) / 6, y: p1.y + (p2.y - p0.y) / 6 }
    const c2 = { x: p2.x - (p3.x - p1.x) / 6, y: p2.y - (p3.y - p1.y) / 6 }
    path += `C${c1.x.toFixed(1)} ${c1.y.toFixed(1)} ${c2.x.toFixed(1)} ${c2.y.toFixed(1)} ${p2.x.toFixed(1)} ${p2.y.toFixed(1)}`
  }

  return path
}

function tickStep(range: number): number {
  if (range > 6) return 2

  return range > 3 ? 1 : 0.5
}

/** A drawing as tall as the figure is wide allows: lower on a phone, taller on the wall. */
function heightFor(width: number): number {
  return width < 520 ? 250 : 340
}

/**
 * Body weight as weigh-in dots under a smoothed trend, on bands of alternating tone instead of
 * gridlines; touch a day to read its value.
 */
export function WeightChart({
  history,
}: {
  readonly history: ReadonlyArray<BodyWeight>
}): React.JSX.Element | null {
  const [focus, setFocus] = useState<number>()
  const [width, setWidth] = useState(defaultWidth)
  const figure = useRef<HTMLElement>(null)

  useEffect(() => {
    const element = figure.current

    if (element === null) return

    const measure = () => {
      if (element.clientWidth > 0) setWidth(Math.round(element.clientWidth))
    }

    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(element)

    return () => observer.disconnect()
  }, [])

  if (history.length === 0) return null

  const height = heightFor(width)
  const first = time(history[0]?.date ?? "")
  const last = time(history.at(-1)?.date ?? "")
  const span = Math.max(day, last - first)
  const weights = history.map((entry) => entry.weight_kg)
  const low = Math.floor(Math.min(...weights) * 2 - 1) / 2
  const high = Math.ceil(Math.max(...weights) * 2 + 1) / 2
  const plotWidth = width - pad.left - pad.right
  const plotHeight = height - pad.top - pad.bottom
  const x = (at: number) => pad.left + ((at - first) / span) * plotWidth
  const y = (kg: number) => pad.top + ((high - kg) / (high - low)) * plotHeight

  const points: Point[] = history.map((entry) => {
    const at = time(entry.date)

    const near = history.filter(
      (other) => Math.abs(time(other.date) - at) <= smoothingDays * day
    )

    const trend =
      near.reduce((sum, other) => sum + other.weight_kg, 0) / near.length

    return { x: x(at), y: y(entry.weight_kg), entry, trend }
  })

  const trendLine = smooth(
    points.map((point) => ({ x: point.x, y: y(point.trend) }))
  )

  const step = tickStep(high - low)
  const ticks: number[] = []

  for (let kg = Math.ceil(low / step) * step; kg <= high; kg += step)
    ticks.push(kg)

  // A band is the stretch between two ticks; every second one is a darker tone.
  const bands = ticks.map((kg, index) => ({
    kg,
    top: y(Math.min(high, kg + step)),
    bottom: y(Math.max(low, kg)),
    dark: index % 2 === 0,
  }))

  const months: Array<{ at: number; label: string }> = []
  const cursor = new Date(first)
  cursor.setUTCDate(1)
  cursor.setUTCMonth(cursor.getUTCMonth() + 1)

  while (cursor.getTime() <= last) {
    months.push({ at: cursor.getTime(), label: monthName.format(cursor) })
    cursor.setUTCMonth(cursor.getUTCMonth() + 1)
  }

  const latest = points.at(-1)

  const lightest = points.reduce((best, point) =>
    point.entry.weight_kg < best.entry.weight_kg ? point : best
  )

  const shown = focus === undefined ? undefined : points[focus]

  return (
    <figure className="weight-chart" ref={figure}>
      <svg
        width={width}
        height={height}
        viewBox={`0 0 ${width} ${height}`}
        aria-hidden="true"
        onPointerMove={(event) => {
          const box = event.currentTarget.getBoundingClientRect()
          const at = ((event.clientX - box.left) / box.width) * width
          let nearest = 0

          for (const [index, point] of points.entries())
            if (
              Math.abs(point.x - at) < Math.abs((points[nearest]?.x ?? 0) - at)
            )
              nearest = index

          setFocus(nearest)
        }}
        onPointerLeave={() => setFocus(undefined)}
      >
        {bands.map((band) => (
          <rect
            key={band.kg}
            className="weight-chart-band"
            data-dark={band.dark}
            x="0"
            width={width}
            y={band.top}
            height={Math.max(0, band.bottom - band.top)}
          />
        ))}
        {ticks.map((kg) => (
          <text
            key={kg}
            className="weight-chart-tick"
            x={width - 16}
            y={y(kg) - 8}
          >
            {kilograms.format(kg)}
          </text>
        ))}
        {months.map((month) => (
          <text
            key={month.at}
            className="weight-chart-month"
            x={x(month.at)}
            y={height - 12}
          >
            {month.label}
          </text>
        ))}
        <path className="weight-chart-trend" d={trendLine} />
        {points.map((point) => (
          <circle
            key={point.entry.id}
            className="weight-chart-dot"
            cx={point.x}
            cy={point.y}
            r="3.5"
          />
        ))}
        {points.length > 2 ? (
          <g className="weight-chart-low">
            <circle cx={lightest.x} cy={lightest.y} r="6" />
            <text x={lightest.x} y={lightest.y + 24}>
              {kilograms.format(lightest.entry.weight_kg)}
            </text>
          </g>
        ) : null}
        {latest === undefined ? null : (
          <circle
            className="weight-chart-latest"
            cx={latest.x}
            cy={latest.y}
            r="8"
          />
        )}
        {shown === undefined ? null : (
          <circle
            className="weight-chart-focus"
            cx={shown.x}
            cy={shown.y}
            r="7"
          />
        )}
      </svg>
      <figcaption className="weight-chart-readout" aria-hidden="true">
        {shown === undefined ? null : (
          <>
            <strong>{kilograms.format(shown.entry.weight_kg)}</strong>
            <span>{shortDate.format(new Date(time(shown.entry.date)))}</span>
          </>
        )}
      </figcaption>
    </figure>
  )
}
