import type { NoopSelection } from "@/lib/training"

export const clock = new Intl.DateTimeFormat("en-GB", {
  timeZone: "Europe/Copenhagen",
  hour: "2-digit",
  minute: "2-digit",
  hourCycle: "h23",
})

export function noopButtonId(selection: NoopSelection): string {
  return `noop-workout-${JSON.stringify([selection.source, selection.start, selection.sport])}`
}

export const capsLabel =
  "text-caption font-semibold tracking-wider text-muted-ink uppercase"

export const clockTimes =
  "flex justify-between text-clock font-semibold tracking-figure-tight tabular-nums *:inline-flex *:items-center *:gap-2 [&_svg]:text-muted-ink"
