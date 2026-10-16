import type { Icon } from "@phosphor-icons/react/dist/lib/types"
import type { Band } from "@/lib/recovery-score"

/** Which measurement a ring draws; each has its own colour family. */
export type RingKind = "charge" | "rest" | "effort"

type ScoreRingProps = {
  readonly kind: RingKind
  /** The filled fraction, 0 through 1. */
  readonly share: number
  /** Colour band; Effort is never good or bad, so it ignores the band unless missing. */
  readonly band: Band
  /** The figure inside the ring; null draws a dash. */
  readonly figure: string | null
  /** A small unit after the figure, such as "%". */
  readonly unit?: string
  /** The visible one-word name below the ring. */
  readonly label: string
  /** The full reading for assistive technology. */
  readonly spoken: string
  /** A glyph above the figure that names the ring without a word. */
  readonly icon?: Icon
}

const radius = 52

const circumference = 2 * Math.PI * radius

/** A WHOOP-style score: a coloured arc around the figure, filled by how far the value goes. */
export function ScoreRing({
  kind,
  share,
  band,
  figure,
  unit,
  label,
  spoken,
  icon: RingIcon,
}: ScoreRingProps): React.JSX.Element {
  return (
    <figure
      className="score-ring"
      data-kind={kind}
      data-band={band}
      role="img"
      aria-label={`${label}: ${spoken}`}
    >
      <svg viewBox="0 0 120 120" aria-hidden="true">
        <circle className="score-ring-track" cx="60" cy="60" r={radius} />
        {share > 0 ? (
          <circle
            className="score-ring-arc"
            cx="60"
            cy="60"
            r={radius}
            strokeDasharray={`${circumference} ${circumference}`}
            // SAFETY: CSSProperties omits custom properties; both are SVG user units read by .score-ring-arc.
            style={
              {
                "--ring-offset": circumference * (1 - share),
                "--ring-length": circumference,
              } as React.CSSProperties
            }
          />
        ) : null}
      </svg>
      <div className="score-ring-figure" aria-hidden="true">
        {RingIcon === undefined ? null : (
          <RingIcon className="score-ring-icon" weight="fill" />
        )}
        <span data-gap={figure === null}>{figure ?? "–"}</span>
        {figure !== null && unit !== undefined ? <small>{unit}</small> : null}
      </div>
      <figcaption aria-hidden="true">{label}</figcaption>
    </figure>
  )
}

type DayRingsProps = {
  readonly charge: { readonly share: number; readonly band: Band }
  readonly rest: { readonly share: number; readonly band: Band }
  readonly effort: { readonly share: number }
}

/** Three concentric arcs for one day: Charge outside, Rest in the middle, Effort inside. */
export function DayRings({
  charge,
  rest,
  effort,
}: DayRingsProps): React.JSX.Element {
  const rings = [
    { kind: "charge", r: 21, ...charge },
    { kind: "rest", r: 15, ...rest },
    { kind: "effort", r: 9, band: "fair" as const, ...effort },
  ] as const

  return (
    <svg className="day-rings" viewBox="0 0 50 50" aria-hidden="true">
      {rings.map((ring) => {
        const length = 2 * Math.PI * ring.r

        return (
          <g key={ring.kind} data-kind={ring.kind} data-band={ring.band}>
            <circle className="day-rings-track" cx="25" cy="25" r={ring.r} />
            {ring.share > 0 ? (
              <circle
                className="day-rings-arc"
                cx="25"
                cy="25"
                r={ring.r}
                strokeDasharray={`${length * ring.share} ${length}`}
              />
            ) : null}
          </g>
        )
      })}
    </svg>
  )
}
