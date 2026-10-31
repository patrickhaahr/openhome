import { useEffect, useRef, useState } from "react"

/** A series' colour, from the design tokens through CSS. */
export type Tone = "ink" | "sun" | "fat" | "muted" | "faint"

type Values = ReadonlyArray<number | null>

/** One named series of values, one per period; null is a gap. */
export type Series = {
  readonly key: string
  readonly name: string
  readonly tone: Tone
  readonly values: Values
  /** What the table says for a period without a value; "No value" by default. */
  readonly gap?: string
}

/** How a chart draws its series. */
export type Mark =
  | ({ readonly kind: "bars" } & Series)
  | { readonly kind: "stack"; readonly parts: readonly Series[] }
  | ({ readonly kind: "line"; readonly dashed?: boolean } & Series)
  | ({ readonly kind: "dots" } & Series)

type TrendChartProps = {
  /** Names the chart for styling and tests. */
  readonly chart: string
  readonly title: string
  /** One line over the whole range, shown and read before the chart. */
  readonly summary: string
  /** Short axis label per period. */
  readonly labels: readonly string[]
  /** Full name per period, for the table. */
  readonly names: readonly string[]
  /** Periods that cover only part of their week or month, drawn lighter. */
  readonly partial: readonly boolean[]
  readonly marks: readonly Mark[]
  /** Formats a value for the table, with its unit. */
  readonly format: (value: number) => string
  /** Formats an axis value; `format` by default. */
  readonly tick?: (value: number) => string
  /** Keep zero on the value axis, as bars need. */
  readonly zero?: boolean
  /** A fixed value axis, such as 0 to 100 for a share. */
  readonly domain?: readonly [number, number]
}

/** The width drawn before the figure is measured. */
const defaultWidth = 640

const pad = { top: 14, right: 60, bottom: 30, left: 8 }

/** Axis labels across the bottom at most. */
const maxLabels = 6

function heightFor(width: number): number {
  return width < 520 ? 170 : 210
}

function seriesOf(marks: readonly Mark[]): Series[] {
  return marks.flatMap((mark) => (mark.kind === "stack" ? mark.parts : [mark]))
}

function finite(values: Values): number[] {
  return values.filter((value): value is number => value !== null)
}

/** Each period's stacked total; null when no part has a value. */
function stackTotals(parts: readonly Series[], count: number): Values {
  return Array.from({ length: count }, (_, index) => {
    const known = finite(parts.map((part) => part.values[index] ?? null))

    return known.length === 0 ? null : known.reduce((sum, value) => sum + value)
  })
}

/** A round step giving about four bands between `low` and `high`. */
function niceStep(low: number, high: number): number {
  const raw = (high - low) / 4
  const magnitude = 10 ** Math.floor(Math.log10(raw))
  const scaled = raw / magnitude
  const nice = [1, 2, 2.5, 5].find((step) => scaled <= step) ?? 10

  return nice * magnitude
}

type Scale = {
  readonly low: number
  readonly high: number
  readonly step: number
}

function scaleFor(props: TrendChartProps): Scale {
  if (props.domain !== undefined) {
    const [low, high] = props.domain

    return { low, high, step: niceStep(low, high) }
  }

  const values = props.marks.flatMap((mark) =>
    mark.kind === "stack"
      ? finite(stackTotals(mark.parts, props.labels.length))
      : finite(mark.values)
  )

  let low = Math.min(...values, ...(props.zero === true ? [0] : []))
  let high = Math.max(...values, ...(props.zero === true ? [0] : []))

  if (!Number.isFinite(low) || !Number.isFinite(high))
    return { low: 0, high: 1, step: 0.25 }

  if (high - low < 1e-9) {
    const margin = Math.max(1, Math.abs(high) * 0.05)
    low = props.zero === true && low >= 0 ? 0 : low - margin
    high += margin
  }

  const step = niceStep(low, high)

  return {
    low: Math.floor(low / step + 1e-9) * step,
    high: Math.ceil(high / step - 1e-9) * step,
    step,
  }
}

/** Unbroken runs of known values, so a line stops at a gap and resumes after it. */
function runs(values: Values): Array<Array<{ index: number; value: number }>> {
  const all: Array<Array<{ index: number; value: number }>> = []
  let current: Array<{ index: number; value: number }> = []

  for (const [index, value] of values.entries()) {
    if (value === null) {
      if (current.length > 0) all.push(current)
      current = []
      continue
    }

    current.push({ index, value })
  }

  if (current.length > 0) all.push(current)

  return all
}

function useWidth(): [React.RefObject<HTMLElement | null>, number] {
  const element = useRef<HTMLElement>(null)
  const [width, setWidth] = useState(defaultWidth)

  useEffect(() => {
    const figure = element.current

    if (figure === null) return

    const measure = () => {
      if (figure.clientWidth > 0) setWidth(Math.round(figure.clientWidth))
    }

    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(figure)

    return () => observer.disconnect()
  }, [])

  return [element, width]
}

/**
 * One Trends series as hand-drawn SVG on bands of alternating tone instead of gridlines: bars
 * rise from zero, lines break where a period has no value, and a gap stays empty. The drawing is
 * hidden from assistive technology, which reads the summary and a table of every period instead.
 */
export function TrendChart(props: TrendChartProps): React.JSX.Element {
  const [figure, width] = useWidth()
  const { labels, marks, format } = props
  const tick = props.tick ?? format
  const count = labels.length
  const height = heightFor(width)
  const scale = scaleFor(props)
  const plotWidth = width - pad.left - pad.right
  const plotHeight = height - pad.top - pad.bottom
  const slot = plotWidth / Math.max(1, count)
  const barWidth = Math.max(1, Math.min(slot * 0.66, 26))
  const x = (index: number) => pad.left + (index + 0.5) * slot

  const y = (value: number) =>
    pad.top + ((scale.high - value) / (scale.high - scale.low)) * plotHeight

  const base = y(Math.min(Math.max(0, scale.low), scale.high))

  const ticks: number[] = []

  for (let value = scale.low; value <= scale.high + 1e-9; value += scale.step)
    // Adding zero turns a rounded -0 into 0, so the axis never reads "−0".
    ticks.push(Number(value.toFixed(6)) + 0)

  const labelEvery = Math.max(1, Math.ceil(count / maxLabels))
  const series = seriesOf(marks)

  const bar = (
    key: string,
    tone: Tone,
    index: number,
    from: number,
    to: number
  ) => (
    <rect
      key={`${key}-${index}`}
      data-mark={key}
      data-tone={tone}
      data-partial={props.partial[index] === true ? "" : undefined}
      x={x(index) - barWidth / 2}
      width={barWidth}
      y={Math.min(from, to)}
      height={Math.abs(to - from)}
    />
  )

  return (
    <figure className="trend-chart" data-chart={props.chart} ref={figure}>
      <figcaption>
        <h3>{props.title}</h3>
        <p className="trend-summary">{props.summary}</p>
        <ul className="trend-legend" aria-hidden="true">
          {marks.flatMap((mark) =>
            (mark.kind === "stack" ? mark.parts : [mark]).map((part) => (
              <li
                key={part.key}
                data-tone={part.tone}
                data-kind={
                  mark.kind === "line" && mark.dashed === true
                    ? "dashed"
                    : mark.kind
                }
              >
                {part.name}
              </li>
            ))
          )}
        </ul>
      </figcaption>
      <svg
        width={width}
        height={height}
        viewBox={`0 0 ${width} ${height}`}
        aria-hidden="true"
      >
        {ticks.slice(0, -1).map((value, index) => (
          <rect
            key={`band-${value}`}
            className="trend-band"
            data-dark={index % 2 === 0}
            x="0"
            width={width}
            y={y(ticks[index + 1] ?? scale.high)}
            height={Math.max(0, y(value) - y(ticks[index + 1] ?? scale.high))}
          />
        ))}
        {ticks.map((value) => (
          <text
            key={`tick-${value}`}
            className="trend-tick"
            x={width - 10}
            y={y(value) - 5}
          >
            {tick(value)}
          </text>
        ))}
        {labels.map((label, index) =>
          index % labelEvery === 0 ? (
            <text
              key={`label-${index}`}
              className="trend-label"
              x={x(index)}
              y={height - 10}
            >
              {label}
            </text>
          ) : null
        )}
        {marks.map((mark) => {
          switch (mark.kind) {
            case "bars":
              return (
                <g key={mark.key}>
                  {mark.values.map((value, index) =>
                    value === null
                      ? null
                      : bar(mark.key, mark.tone, index, base, y(value))
                  )}
                </g>
              )
            case "stack":
              return (
                <g key={mark.parts.map((part) => part.key).join("-")}>
                  {Array.from({ length: count }, (_, index) => {
                    let top = 0

                    return mark.parts.map((part) => {
                      const value = part.values[index] ?? null

                      if (value === null || value <= 0) return null
                      const from = y(top)
                      top += value

                      return bar(part.key, part.tone, index, from, y(top))
                    })
                  })}
                </g>
              )
            case "line":
              return (
                <g key={mark.key} data-mark={mark.key} data-tone={mark.tone}>
                  {runs(mark.values).map((run) =>
                    run.length === 1 ? (
                      <circle
                        key={run[0]?.index}
                        className="trend-point"
                        cx={x(run[0]?.index ?? 0)}
                        cy={y(run[0]?.value ?? 0)}
                        r="3"
                      />
                    ) : (
                      <path
                        key={run[0]?.index}
                        className="trend-line"
                        data-dashed={mark.dashed === true ? "" : undefined}
                        d={run
                          .map(
                            (point, position) =>
                              `${position === 0 ? "M" : "L"}${x(point.index).toFixed(1)} ${y(point.value).toFixed(1)}`
                          )
                          .join("")}
                      />
                    )
                  )}
                </g>
              )
            case "dots":
              return (
                <g key={mark.key} data-mark={mark.key} data-tone={mark.tone}>
                  {mark.values.map((value, index) =>
                    value === null ? null : (
                      <circle
                        key={index}
                        className="trend-dot"
                        cx={x(index)}
                        cy={y(value)}
                        r="3"
                      />
                    )
                  )}
                </g>
              )
          }
        })}
      </svg>
      <table className="sr-only">
        <caption>{props.title}</caption>
        <thead>
          <tr>
            <th scope="col">Period</th>
            {series.map((part) => (
              <th key={part.key} scope="col">
                {part.name}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {props.names.map((name, index) => (
            <tr key={name}>
              <th scope="row">{name}</th>
              {series.map((part) => {
                const value = part.values[index] ?? null

                return (
                  <td key={part.key}>
                    {value === null ? (part.gap ?? "No value") : format(value)}
                  </td>
                )
              })}
            </tr>
          ))}
        </tbody>
      </table>
    </figure>
  )
}
