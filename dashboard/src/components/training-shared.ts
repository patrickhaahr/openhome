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
