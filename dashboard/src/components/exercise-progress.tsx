import { useId, useRef, useState } from "react"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader } from "@/components/ui/card"
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import {
  ViewBar,
  WorkoutFeedback,
  workoutMuted,
} from "@/components/workout-parts"
import {
  hue,
  paletteSize,
  ProgressChart,
  type Marker,
} from "@/components/progress-chart"
import {
  byLatestTrained,
  indexValue,
  metricLabel,
  metricPolicy,
  missingLineReason,
  progressFailureMessage,
  progressPeriods,
  rawValue,
  shortDay,
  type ChartedLine,
  type CombinedProgress,
  type ProgressLine,
  type ProgressPeriod,
  type ProgressState,
} from "@/lib/exercise-progress"

type ExerciseProgressProps = {
  readonly data: ProgressState
  readonly choosePeriod: (period: ProgressPeriod) => Promise<void>
  /** Compare exactly these Exercises; an empty list returns to the most recently logged. */
  readonly compare: (ids: ReadonlyArray<number>) => Promise<void>
  /** The view switcher, which sits at the start of the toolbar. */
  readonly lead?: React.ReactNode
}

/** Lines shown before the household toggles any, keeping a first glance readable. */
const initiallyShown = 6

/** Whether each toggled line is shown, by Exercise id; untouched lines use the default. */
type Toggles = ReadonlyMap<number, boolean>

const tallButton = "min-h-12 touch-manipulation"

const cardHeading = "text-xlarge font-semibold"

const cell = "px-2 py-3.5 text-left"

const lastCell = "px-2 py-3.5 text-right tabular-nums"

const headText = "text-caption text-muted-ink"

function isCharted(line: ProgressLine): line is ChartedLine {
  return line.status === "charted"
}

/** Combined, normalized exercise progress with per-line toggles and raw values. */
export function ExerciseProgress({
  data,
  choosePeriod,
  compare,
  lead,
}: ExerciseProgressProps): React.JSX.Element {
  const [busy, setBusy] = useState(false)
  const locked = useRef(false)
  const [failure, setFailure] = useState<string>()

  // Choices outlive reads (period changes, added exercises), so they live above the view.
  const [toggled, setToggled] = useState<Toggles>(() => new Map())

  async function run(work: () => Promise<void>): Promise<void> {
    if (locked.current) return
    locked.current = true
    setBusy(true)
    setFailure(undefined)

    try {
      await work()
    } catch {
      setFailure("Could not update progress. Try again when OpenHome returns.")
    } finally {
      locked.current = false
      setBusy(false)
    }
  }

  const shownIds =
    data.state === "available"
      ? data.progress.exercises.map((line) => line.exercise.id)
      : []

  return (
    <div className="grid" aria-busy={busy}>
      <ViewBar lead={lead}>
        {data.state === "available" && data.compared.length > 0 ? (
          <Button
            className={tallButton}
            variant="ghost"
            disabled={busy}
            onClick={() => {
              void run(() => compare([]))
            }}
          >
            Most recent
          </Button>
        ) : null}
        <ToggleGroup
          aria-label="Progress period"
          variant="capsule"
          size="capsule"
          spacing={1}
          value={data.state === "available" ? [data.period] : []}
          disabled={busy}
          onValueChange={([next]) => {
            if (next === "4w" || next === "3m" || next === "12m")
              void run(() => choosePeriod(next))
          }}
        >
          {(["4w", "3m", "12m"] as const).map((key) => (
            <ToggleGroupItem key={key} value={key}>
              {progressPeriods[key].label}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
      </ViewBar>
      <WorkoutFeedback failed>{failure}</WorkoutFeedback>
      {data.state === "unavailable" ? (
        <Card>
          <CardHeader>
            <h2 className={cardHeading}>Progress unavailable</h2>
          </CardHeader>
          <CardContent>
            <p>{progressFailureMessage[data.reason]}</p>
          </CardContent>
        </Card>
      ) : (
        <ProgressView
          progress={data.progress}
          period={data.period}
          explicit={data.compared.length > 0}
          busy={busy}
          toggled={toggled}
          toggle={(id, shown) =>
            setToggled((current) => new Map(current).set(id, shown))
          }
          remove={(id) => {
            void run(() => compare(shownIds.filter((shown) => shown !== id)))
          }}
        />
      )}
    </div>
  )
}

type ProgressViewProps = {
  readonly progress: CombinedProgress
  readonly period: ProgressPeriod
  readonly explicit: boolean
  readonly busy: boolean
  readonly toggled: Toggles
  readonly toggle: (id: number, shown: boolean) => void
  /** Stop comparing an Exercise; the rest become an explicit comparison. */
  readonly remove: (id: number) => void
}

function ProgressView({
  progress,
  period,
  explicit,
  busy,
  toggled,
  toggle,
  remove,
}: ProgressViewProps): React.JSX.Element {
  const ordered = byLatestTrained(progress.exercises)
  const charted = ordered.filter(isCharted)

  const missing = progress.exercises.filter(
    (line): line is Exclude<ProgressLine, ChartedLine> => !isCharted(line)
  )

  const shown = new Set(
    charted
      .filter(
        (line, position) =>
          toggled.get(line.exercise.id) ?? position < initiallyShown
      )
      .map((line) => line.exercise.id)
  )

  const [inspected, setInspected] = useState<number>()

  // Slots follow the order, newest session first, so toggling, periods and absences never recolour a
  // compared line.
  const markers = new Map(
    ordered.map((line, position): [number, Marker] => [
      line.exercise.id,
      {
        colour: (position % paletteSize) + 1,
        dot: position < paletteSize ? "round" : "square",
      },
    ])
  )

  // Removing the only line of the default selection would change nothing.
  const removable = explicit || progress.exercises.length > 1

  const visible = charted.filter((line) => shown.has(line.exercise.id))

  const inspectedLine =
    charted.find((line) => line.exercise.id === inspected) ?? charted[0]

  const selectId = useId()

  let summary = `${charted.length} charted of ${progress.exercises.length} compared exercises.`

  if (!explicit)
    summary =
      progress.logged_exercise_count > progress.exercises.length
        ? `The ${progress.exercises.length} most recently logged of ${progress.logged_exercise_count} exercises in this period.`
        : `Every exercise logged in this period.`

  return (
    <div className="grid gap-6">
      <Card>
        <CardHeader>
          <div className="flex flex-wrap items-start justify-between gap-4">
            <div>
              <p className="sr-only">
                {progressPeriods[period].label} · baseline = 100
              </p>
              <h2 className="sr-only">Exercise progress</h2>
            </div>
            <p className="sr-only">{summary}</p>
          </div>
        </CardHeader>
        <CardContent>
          {charted.length === 0 ? (
            <div className="grid justify-items-start gap-3 py-10">
              <p className="text-muted-ink">
                {progress.exercises.length === 0
                  ? "Nothing logged in this period. Choose a longer period or log a workout."
                  : "None of these exercises can be charted. Their reasons are listed below."}
              </p>
            </div>
          ) : (
            <>
              <ProgressChart
                from={progress.from}
                to={progress.to}
                lines={visible}
                markers={markers}
              />
              <div
                className="mt-6 grid grid-cols-legend gap-2"
                role="group"
                aria-label="Exercise lines"
              >
                {charted.map((line) => {
                  const latest = line.samples.at(-1)
                  const marker = markers.get(line.exercise.id)
                  const pressed = shown.has(line.exercise.id)

                  return (
                    <button
                      key={line.exercise.id}
                      type="button"
                      className="grid min-h-12 touch-manipulation grid-cols-lead content-start gap-x-3 gap-y-0.5 rounded-md bg-wash p-3 text-left text-muted-ink transition-colors focus-visible:outline-3 focus-visible:outline-offset-2 focus-visible:outline-ink aria-pressed:bg-wash-strong aria-pressed:text-ink motion-reduce:transition-none"
                      aria-pressed={shown.has(line.exercise.id)}
                      onClick={() =>
                        toggle(line.exercise.id, !shown.has(line.exercise.id))
                      }
                    >
                      <span
                        className={`row-span-3 mt-0.5 size-4 border-2 border-current ${hue(marker)} ${marker?.dot === "square" ? "rounded-xs" : "rounded-full"} ${pressed ? "bg-current" : "bg-transparent"}`}
                        aria-hidden="true"
                      />
                      <span
                        data-slot="progress-legend-name"
                        className="col-start-2 font-semibold wrap-anywhere"
                      >
                        {line.exercise.name}
                      </span>
                      <span className="sr-only col-start-2">
                        {metricLabel(line)} · baseline{" "}
                        {rawValue(line, line.baseline.value)} on{" "}
                        {shortDay(line.baseline.date)}
                      </span>
                      {latest === undefined ? null : (
                        <span className="col-start-2 text-label tabular-nums">
                          <span className="sr-only">Latest </span>
                          {rawValue(line, latest.value)} ·{" "}
                          {indexValue(latest.index)}
                        </span>
                      )}
                    </button>
                  )
                })}
              </div>
            </>
          )}
        </CardContent>
      </Card>
      <div className="grid grid-cols-2 items-start gap-6 max-[900px]:grid-cols-1">
        {inspectedLine === undefined ? null : (
          <Card>
            <CardHeader>
              <p className="sr-only">Observed values</p>
              <label htmlFor={selectId} className="sr-only">
                Exercise to inspect
              </label>
              <div className="flex items-center gap-2">
                <NativeSelect
                  id={selectId}
                  size="field"
                  className="w-full min-w-0 flex-1"
                  value={String(inspectedLine.exercise.id)}
                  onChange={(event) => setInspected(Number(event.target.value))}
                >
                  {charted.map((line) => (
                    <NativeSelectOption
                      key={line.exercise.id}
                      value={line.exercise.id}
                    >
                      {line.exercise.name}
                    </NativeSelectOption>
                  ))}
                </NativeSelect>
                {removable ? (
                  <Button
                    className={tallButton}
                    variant="ghost"
                    disabled={busy}
                    aria-label={`Stop comparing ${inspectedLine.exercise.name}`}
                    onClick={() => remove(inspectedLine.exercise.id)}
                  >
                    Remove
                  </Button>
                ) : null}
              </div>
            </CardHeader>
            <CardContent>
              <p className="sr-only">{metricPolicy[inspectedLine.metric]}</p>
              {inspectedLine.omitted_dates.length > 0 ? (
                <p className={workoutMuted}>
                  Not estimated:{" "}
                  {inspectedLine.omitted_dates.map(shortDay).join(", ")}
                  <span className="sr-only">
                    . No valid value for this measure; these days have no dot.
                  </span>
                </p>
              ) : null}
              <table className="mt-4 w-full">
                <thead>
                  <tr>
                    <th scope="col" className={`${cell} ${headText}`}>
                      Day
                    </th>
                    <th scope="col" className={`${cell} ${headText}`}>
                      Value
                    </th>
                    <th scope="col" className={`${lastCell} ${headText}`}>
                      Index
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {inspectedLine.samples.map((sample) => (
                    <tr key={sample.date}>
                      <th scope="row" className={`${cell} font-normal`}>
                        {shortDay(sample.date)}
                        {sample.date === inspectedLine.baseline.date
                          ? " · baseline"
                          : ""}
                      </th>
                      <td className={cell}>
                        {rawValue(inspectedLine, sample.value)}
                      </td>
                      <td className={lastCell}>{indexValue(sample.index)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </CardContent>
          </Card>
        )}
        {missing.length === 0 ? null : (
          <Card>
            <CardHeader>
              <p className="sr-only">No line</p>
              <h2 className={cardHeading}>Not charted</h2>
            </CardHeader>
            <CardContent>
              <ul data-slot="progress-missing" className="grid">
                {missing.map((line) => (
                  <li
                    key={line.exercise.id}
                    className="flex items-center justify-between gap-4 py-3"
                  >
                    <div className="grid min-w-0 gap-0.5 wrap-anywhere">
                      <span>{line.exercise.name}</span>
                      <span className={workoutMuted}>
                        {missingLineReason(line)}
                      </span>
                    </div>
                    {removable ? (
                      <Button
                        className={tallButton}
                        variant="ghost"
                        disabled={busy}
                        aria-label={`Stop comparing ${line.exercise.name}`}
                        onClick={() => remove(line.exercise.id)}
                      >
                        Remove
                      </Button>
                    ) : null}
                  </li>
                ))}
              </ul>
            </CardContent>
          </Card>
        )}
      </div>
    </div>
  )
}
