import type { Icon } from "@phosphor-icons/react/dist/lib/types"
import type { Band } from "@/lib/recovery-score"

/** Which measurement a ring draws; each has its own colour family. */
export type RingKind = "charge" | "rest" | "effort"

/** Where the ring sits: the Recovery hero, the Home glance, a session's strap or a detail head. */
export type RingSize = "hero" | "glance" | "strap" | "detail"

/** Which 90 ms step of a row's fill the ring starts on; only the hero staggers. */
export type RingStagger = 0 | 1 | 2

type ScoreRingProps = {
  readonly kind: RingKind
  readonly size: RingSize
  readonly stagger?: RingStagger
  /** The filled fraction, 0 through 1. */
  readonly share: number
  /** Colour band; Effort is never good or bad, so it ignores the band unless missing. */
  readonly band: Band
  /** The figure inside the ring; null draws a dash. */
  readonly figure: string | null
  /** The visible one-word name below the ring. */
  readonly label: string
  /** The full reading for assistive technology. */
  readonly spoken: string
  /** A glyph above the figure that names the ring without a word. */
  readonly icon?: Icon
}

type RingLook = {
  readonly root: string
  readonly stroke: number
  readonly figure: string
  readonly value: string
  readonly figureGap: string
  readonly caption: string
}

const sizes: Record<RingSize, RingLook> = {
  hero: {
    root: "gap-3",
    stroke: 9,
    figure: "flex-col items-center gap-1",
    value: "text-score tracking-score",
    figureGap: "data-[gap=true]:gap-dash-5.5",
    caption: "hidden",
  },
  glance: {
    root: "w-36 gap-3 max-sm:w-glance-ring",
    stroke: 9,
    figure: "flex-col items-center gap-0.5",
    value: "text-figure-lg tracking-score-tight max-sm:text-figure-sm",
    figureGap: "",
    caption: "hidden",
  },
  strap: {
    root: "w-27 flex-none gap-1",
    stroke: 11,
    figure: "items-baseline pt-ring-baseline",
    value: "text-figure-md tracking-score",
    figureGap: "",
    caption: "text-caption",
  },
  detail: {
    root: "w-26 gap-1",
    stroke: 11,
    figure: "items-baseline pt-ring-baseline",
    value: "text-figure-sm tracking-score",
    figureGap: "data-[gap=true]:gap-dash-5.5",
    caption: "text-micro",
  },
}

const fills = [
  "motion-safe:animate-score-fill",
  "motion-safe:animate-score-fill-2",
  "motion-safe:animate-score-fill-3",
] as const

const radius = 52

const circumference = 2 * Math.PI * radius

/** A WHOOP-style score: a coloured arc around the figure, filled by how far the value goes. */
export function ScoreRing({
  kind,
  size,
  stagger = 0,
  share,
  band,
  figure,
  label,
  spoken,
  icon: RingIcon,
}: ScoreRingProps): React.JSX.Element {
  const look = sizes[size]

  return (
    <figure
      data-slot="score-ring"
      className={`relative grid justify-items-center ${look.root}`}
      data-kind={kind}
      data-band={band}
      role="img"
      aria-label={`${label}: ${spoken}`}
    >
      <svg
        className="aspect-square w-full -rotate-90 overflow-visible"
        viewBox="0 0 120 120"
        aria-hidden="true"
      >
        <circle
          className="stroke-wash-strong"
          cx="60"
          cy="60"
          r={radius}
          fill="none"
          strokeWidth={look.stroke}
        />
        {share > 0 ? (
          <circle
            className={`${kind === "effort" ? "stroke-effort" : "stroke-band"} ${fills[stagger]}`}
            cx="60"
            cy="60"
            r={radius}
            fill="none"
            strokeWidth={look.stroke}
            strokeLinecap="round"
            strokeDasharray={`${circumference} ${circumference}`}
            strokeDashoffset={circumference * (1 - share)}
            // SAFETY: CSSProperties omits custom properties; the fill animation starts from this SVG user length.
            style={{ "--ring-length": circumference } as React.CSSProperties}
          />
        ) : null}
      </svg>
      <div
        className={`absolute inset-x-0 top-0 flex aspect-square justify-center leading-none tabular-nums ${look.figure}`}
        aria-hidden="true"
      >
        {RingIcon === undefined ? null : (
          <RingIcon
            className={`size-7 ${kind === "effort" ? "text-effort" : "text-band"}`}
            weight="fill"
          />
        )}
        <span className="inline-flex items-baseline">
          <span
            data-gap={figure === null}
            className={`font-semibold ${look.value} ${look.figureGap}`}
          >
            {figure ?? "–"}
          </span>
        </span>
      </div>
      <figcaption
        className={`font-semibold tracking-caption text-muted-ink uppercase ${look.caption}`}
        aria-hidden="true"
      >
        {label}
      </figcaption>
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
    <svg
      className="w-day-rings -rotate-90"
      viewBox="0 0 50 50"
      aria-hidden="true"
    >
      {rings.map((ring) => {
        const length = 2 * Math.PI * ring.r

        return (
          <g key={ring.kind} data-kind={ring.kind} data-band={ring.band}>
            <circle
              className="stroke-wash-strong"
              cx="25"
              cy="25"
              r={ring.r}
              fill="none"
              strokeWidth={4.5}
            />
            {ring.share > 0 ? (
              <circle
                className={
                  ring.kind === "effort" ? "stroke-effort" : "stroke-band"
                }
                cx="25"
                cy="25"
                r={ring.r}
                fill="none"
                strokeWidth={4.5}
                strokeLinecap="round"
                strokeDasharray={`${length * ring.share} ${length}`}
              />
            ) : null}
          </g>
        )
      })}
    </svg>
  )
}
