import { cn } from "@/lib/utils"

/** What a legend swatch can be coloured as; the chart draws only the first five. */
export type LegendTone =
  | "ink"
  | "sun"
  | "fat"
  | "muted"
  | "faint"
  | "danger"
  | "caution"

/** How a swatch is drawn: a square, a line, a dashed line or a dot. */
export type LegendKind = "bars" | "stack" | "line" | "dots" | "dashed"

/** A plate's title. */
export const trendTitle = "text-large font-semibold tracking-figure"

/** The one line under a plate's title. */
export const trendSummary = "text-control text-muted-ink tabular-nums"

const fills = {
  ink: "before:bg-ink",
  sun: "before:bg-sun",
  fat: "before:bg-chart-4",
  muted: "before:bg-muted-ink",
  faint: "before:bg-ink/35",
  danger: "before:bg-danger",
  caution: "before:bg-food-caution",
} satisfies Record<LegendTone, string>

const strokes = {
  ink: "before:border-ink",
  sun: "before:border-sun",
  fat: "before:border-chart-4",
  muted: "before:border-muted-ink",
  faint: "before:border-ink/35",
  danger: "before:border-danger",
  caution: "before:border-food-caution",
} satisfies Record<LegendTone, string>

const kinds = {
  bars: "before:size-3 before:rounded-food-swatch",
  stack: "before:size-3 before:rounded-food-swatch",
  line: "before:h-0.75 before:w-3 before:rounded-full",
  dots: "before:size-2 before:rounded-full",
  dashed: "before:h-0 before:w-3 before:border-t-3 before:border-dashed",
} satisfies Record<LegendKind, string>

/** One swatch and its name; a dashed swatch is a border, so it takes no fill. */
function swatch(tone: LegendTone, kind: LegendKind): string {
  return cn(kinds[kind], kind === "dashed" ? strokes[tone] : fills[tone])
}

/** A key to a plate's marks, hidden from assistive technology like the drawing it explains. */
export function TrendLegend({
  items,
}: {
  readonly items: ReadonlyArray<{
    readonly key: string
    readonly name: string
    readonly tone: LegendTone
    readonly kind: LegendKind
  }>
}): React.JSX.Element {
  return (
    <ul
      className="flex flex-wrap gap-x-4 gap-y-1 text-label text-muted-ink"
      aria-hidden="true"
    >
      {items.map((item) => (
        <li
          key={item.key}
          className={cn(
            "inline-flex items-center gap-1.5 before:flex-none",
            swatch(item.tone, item.kind)
          )}
        >
          {item.name}
        </li>
      ))}
    </ul>
  )
}
