type SparklineProps = {
  /** Oldest first; a null value is a day with nothing recorded, bridged by the line. */
  readonly points: ReadonlyArray<{ readonly value: number | null }>
  /** A reference drawn as a dashed line, such as the recent average. */
  readonly baseline: number | null
}

/** A small trend line with a dot on the latest recorded value. Decorative; values stay in text. */
export function Sparkline({
  points,
  baseline,
}: SparklineProps): React.JSX.Element | null {
  const values = points.flatMap((point) =>
    point.value === null ? [] : [point.value]
  )

  if (values.length < 2) return null

  const low = Math.min(...values, baseline ?? Infinity)
  const high = Math.max(...values, baseline ?? -Infinity)
  const range = high - low || 1
  const step = 192 / Math.max(1, points.length - 1)
  const y = (value: number) => 42 - ((value - low) / range) * 36

  // A glance shows the shape; a gap of unrecorded days is bridged rather than left as islands.
  const marks = points.flatMap((point, index) =>
    point.value === null ? [] : [{ x: 4 + index * step, y: y(point.value) }]
  )

  const path = marks
    .map(
      (at, index) =>
        `${index === 0 ? "M" : "L"}${at.x.toFixed(2)} ${at.y.toFixed(2)}`
    )
    .join("")

  const last = marks.at(-1)

  return (
    <svg className="sparkline" viewBox="0 0 200 48" aria-hidden="true">
      {baseline === null ? null : (
        <line
          className="sparkline-baseline"
          x1="4"
          x2="196"
          y1={y(baseline)}
          y2={y(baseline)}
        />
      )}
      <path className="sparkline-line" d={path} />
      {last === undefined ? null : (
        <circle className="sparkline-dot" cx={last.x} cy={last.y} r="3.5" />
      )}
    </svg>
  )
}
