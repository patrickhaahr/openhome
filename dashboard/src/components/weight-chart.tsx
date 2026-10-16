import { useState } from "react"
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

const width = 1000

const height = 300

const pad = { top: 24, right: 56, bottom: 34, left: 8 }

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

/** Body weight as weigh-in dots under a smoothed trend; touch a day to read its value. */
export function WeightChart({
  history,
}: {
  readonly history: ReadonlyArray<BodyWeight>
}): React.JSX.Element | null {
  const [focus, setFocus] = useState<number>()

  if (history.length === 0) return null

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

  const area =
    points.length < 2
      ? ""
      : `${trendLine}L${points.at(-1)?.x ?? 0} ${height - pad.bottom}L${points[0]?.x ?? 0} ${height - pad.bottom}Z`

  const step = tickStep(high - low)
  const ticks: number[] = []

  for (let kg = Math.ceil(low / step) * step; kg <= high; kg += step)
    ticks.push(kg)

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
    <figure className="weight-chart">
      <svg
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
        {ticks.map((kg) => (
          <g key={kg} className="weight-chart-tick">
            <line x1={pad.left} x2={width - pad.right} y1={y(kg)} y2={y(kg)} />
            <text x={width - pad.right + 12} y={y(kg) + 5}>
              {kilograms.format(kg)}
            </text>
          </g>
        ))}
        {months.map((month) => (
          <text
            key={month.at}
            className="weight-chart-month"
            x={x(month.at)}
            y={height - 8}
          >
            {month.label}
          </text>
        ))}
        <path className="weight-chart-area" d={area} />
        <path className="weight-chart-trend" d={trendLine} />
        {points.map((point) => (
          <circle
            key={point.entry.id}
            className="weight-chart-dot"
            cx={point.x}
            cy={point.y}
            r="4"
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
          <g className="weight-chart-focus">
            <line
              x1={shown.x}
              x2={shown.x}
              y1={pad.top}
              y2={height - pad.bottom}
            />
            <circle cx={shown.x} cy={shown.y} r="7" />
          </g>
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
