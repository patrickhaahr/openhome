import type { ChartedLine } from "./exercise-progress"
import { getProgress } from "./exercise-progress.functions"
import type { RecoveryDay } from "./training"
import { getRecoveryToday } from "./training.functions"

/** Quiet lines Home draws behind the featured ones; Axum orders them most recent first. */
const quietLines = 5

/**
 * Exercises Home names in colour, matched by name. Unmatched slots go to the biggest gains,
 * so a renamed or skipped exercise never leaves the chart without colour.
 */
const featuredNames = ["back row", "pull-up", "chest press", "dip"] as const

/** The last four weeks of exercise progress, only lines with a trend to draw. */
export type ProgressGlance = {
  readonly from: string
  readonly to: string
  /** Up to four lines drawn in colour and named in the legend. */
  readonly featured: readonly ChartedLine[]
  readonly others: readonly ChartedLine[]
}

function gain(line: ChartedLine): number {
  return (line.samples.at(-1)?.index ?? 100) - 100
}

function feature(lines: readonly ChartedLine[]): ChartedLine[] {
  const named = featuredNames.flatMap((name) => {
    const match = lines.find((line) =>
      line.exercise.name.toLowerCase().includes(name)
    )

    return match === undefined ? [] : [match]
  })

  const rest = lines
    .filter((line) => !named.includes(line))
    .sort((a, b) => gain(b) - gain(a))

  return [...named, ...rest].slice(0, featuredNames.length)
}

/**
 * Home's live values, each deferred and independent, so no read delays the clock or the
 * controls. `null` means the value could not be read, never "empty".
 */
export type Glance = {
  readonly progress: Promise<ProgressGlance | null>
  readonly recovery: Promise<RecoveryDay | null>
}

export function loadGlance(): Glance {
  return {
    progress: getProgress({ data: { period: "4w" } })
      .then((read) => {
        if (read.state !== "available") return null

        const lines = read.progress.exercises.filter(
          (line): line is ChartedLine =>
            line.status === "charted" && line.samples.length > 1
        )

        const featured = feature(lines)

        return {
          from: read.progress.from,
          to: read.progress.to,
          featured,
          others: lines
            .filter((line) => !featured.includes(line))
            .slice(0, quietLines),
        }
      })
      .catch(() => null),
    recovery: getRecoveryToday()
      .then((read) => (read.state === "available" ? read.data : null))
      .catch(() => null),
  }
}
