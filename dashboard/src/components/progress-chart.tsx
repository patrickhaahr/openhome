import { useEffect, useRef, useState } from "react"
import {
  chartScale,
  isGap,
  plot,
  shortDay,
  type ChartedLine,
} from "@/lib/exercise-progress"

/** Twelve hues at similar lightness so no line dominates on dark; later lines reuse them with square markers. */
const hues = [
  "text-progress-1",
  "text-progress-2",
  "text-progress-3",
  "text-progress-4",
  "text-progress-5",
  "text-progress-6",
  "text-progress-7",
  "text-progress-8",
  "text-progress-9",
  "text-progress-10",
  "text-progress-11",
  "text-progress-12",
] as const

export const paletteSize = hues.length

/** A line's colour slot (1-based) and marker shape, fixed by its place in the request. */
export type Marker = {
  readonly colour: number
  readonly dot: "round" | "square"
}

/** The text colour that a line's marks, swatch and strokes draw in through `currentColor`. */
export function hue(marker: Marker | undefined): string {
  return marker === undefined ? "" : (hues[marker.colour - 1] ?? "")
}

const axisText = "fill-muted-ink text-small tabular-nums"

const markFace = "fill-card stroke-current"

type ProgressChartProps = {
  readonly from: string
  readonly to: string
  readonly lines: ReadonlyArray<ChartedLine>
  readonly markers: ReadonlyMap<number, Marker>
}

/** Observed days as dots joined in order; long gaps are dashed and nothing is interpolated. */
export function ProgressChart({
  from,
  to,
  lines,
  markers,
}: ProgressChartProps): React.JSX.Element {
  const [measured, setMeasured] = useState<number>(plot.width)
  const figure = useRef<HTMLElement>(null)

  useEffect(() => {
    const element = figure.current

    if (element === null) return

    const measure = () => {
      if (element.clientWidth > 0) setMeasured(Math.round(element.clientWidth))
    }

    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(element)

    return () => observer.disconnect()
  }, [])

  // Drawn at its own pixel width so type stays readable on a phone.
  const size = { width: measured, height: measured < 520 ? 260 : plot.height }
  const scale = chartScale({ from, to, lines, size })

  const right = size.width - plot.right

  const label =
    lines.length === 0
      ? "No exercise lines shown. Turn on a line below."
      : `Progress index from ${shortDay(from)} to ${shortDay(to)} for ${lines.map((line) => line.exercise.name).join(", ")}. Exact values are in Observed values.`

  return (
    <figure className="grid gap-3" ref={figure}>
      <svg
        className="h-auto w-full overflow-visible"
        viewBox={`0 0 ${size.width} ${size.height}`}
        role="img"
        aria-label={label}
      >
        {scale.ticks.slice(1).map((tick, position) => {
          const below = scale.ticks[position]

          if (below === undefined) return null

          return (
            <rect
              key={tick}
              data-slot="progress-band"
              className={position % 2 === 0 ? "fill-wash" : "fill-transparent"}
              x={plot.left}
              y={scale.y(tick)}
              width={right - plot.left}
              height={scale.y(below) - scale.y(tick)}
            />
          )
        })}
        {scale.ticks.map((tick) => (
          <text
            key={tick}
            className={`text-small tabular-nums ${tick === 100 ? "fill-ink" : "fill-muted-ink"}`}
            x={plot.left - 8}
            y={scale.y(tick) + 4}
            textAnchor="end"
          >
            {tick}
          </text>
        ))}
        <text className={axisText} x={plot.left} y={size.height - 6}>
          {shortDay(from)}
        </text>
        <text
          className={axisText}
          x={right}
          y={size.height - 6}
          textAnchor="end"
        >
          {shortDay(to)}
        </text>
        {lines.map((line) => (
          <g
            key={line.exercise.id}
            data-slot="progress-line"
            className={hue(markers.get(line.exercise.id))}
          >
            {line.samples.slice(1).map((sample, position) => {
              const previous = line.samples[position]

              if (previous === undefined) return null

              const gap =
                isGap(previous.date, sample.date) ||
                line.omitted_dates.some(
                  (date) => date > previous.date && date < sample.date
                )

              return (
                <line
                  key={sample.date}
                  className="stroke-current"
                  strokeLinecap="round"
                  strokeWidth={gap ? 2 : 2.5}
                  strokeDasharray={gap ? "2 7" : undefined}
                  x1={scale.x(previous.date)}
                  y1={scale.y(previous.index)}
                  x2={scale.x(sample.date)}
                  y2={scale.y(sample.index)}
                />
              )
            })}
            {line.samples.map((sample) =>
              markers.get(line.exercise.id)?.dot === "square" ? (
                <rect
                  key={sample.date}
                  className={markFace}
                  strokeWidth={2.5}
                  x={scale.x(sample.date) - 4}
                  y={scale.y(sample.index) - 4}
                  width={8}
                  height={8}
                />
              ) : (
                <circle
                  key={sample.date}
                  className={markFace}
                  strokeWidth={2.5}
                  cx={scale.x(sample.date)}
                  cy={scale.y(sample.index)}
                  r={4}
                />
              )
            )}
          </g>
        ))}
      </svg>
      <figcaption className="sr-only">
        Each line is 100 on its baseline: the exercise’s first logged day in
        this period with a value above zero. Dots are logged days; dashed joins
        cross omitted estimates or more than two weeks without a log.
      </figcaption>
    </figure>
  )
}
