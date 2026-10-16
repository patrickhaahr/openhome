import { useEffect, useId, useRef, useState } from "react"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader } from "@/components/ui/card"
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import {
  byLatestTrained,
  chartScale,
  indexValue,
  isGap,
  metricLabel,
  metricPolicy,
  missingLineReason,
  plot,
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

/** Distinct line colours; later lines reuse them with square markers. */
const paletteSize = 12

/** A line's colour slot and marker shape, fixed by its place in the request. */
type Marker = { readonly colour: number; readonly dot: "round" | "square" }

/** Whether each toggled line is shown, by Exercise id; untouched lines use the default. */
type Toggles = ReadonlyMap<number, boolean>

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
    <div className="exercise-progress workouts" aria-busy={busy}>
      <div className="view-bar">
        {lead}
        <div className="view-bar-tools">
          {data.state === "available" && data.compared.length > 0 ? (
            <Button
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
            variant="outline"
            spacing={0}
            value={data.state === "available" ? [data.period] : []}
            disabled={busy}
            onValueChange={([next]) => {
              if (next === "4w" || next === "3m" || next === "12m")
                void run(() => choosePeriod(next))
            }}
          >
            {(["4w", "3m", "12m"] as const).map((key) => (
              <ToggleGroupItem key={key} value={key} size="lg">
                {progressPeriods[key].label}
              </ToggleGroupItem>
            ))}
          </ToggleGroup>
        </div>
      </div>
      <p className="workout-feedback" data-failed="true" role="status">
        {failure}
      </p>
      {data.state === "unavailable" ? (
        <Card>
          <CardHeader>
            <h2>Progress unavailable</h2>
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
    <div className="exercise-progress-layout">
      <Card>
        <CardHeader>
          <div className="workout-detail-header">
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
            <div className="workout-empty">
              <p>
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
                className="progress-legend"
                role="group"
                aria-label="Exercise lines"
              >
                {charted.map((line) => {
                  const latest = line.samples.at(-1)

                  return (
                    <button
                      key={line.exercise.id}
                      type="button"
                      aria-pressed={shown.has(line.exercise.id)}
                      onClick={() =>
                        toggle(line.exercise.id, !shown.has(line.exercise.id))
                      }
                    >
                      <span
                        className="progress-swatch"
                        data-colour={markers.get(line.exercise.id)?.colour}
                        data-dot={markers.get(line.exercise.id)?.dot}
                        aria-hidden="true"
                      />
                      <span className="progress-legend-name">
                        {line.exercise.name}
                      </span>
                      <span className="sr-only">
                        {metricLabel(line)} · baseline{" "}
                        {rawValue(line, line.baseline.value)} on{" "}
                        {shortDay(line.baseline.date)}
                      </span>
                      {latest === undefined ? null : (
                        <span className="progress-legend-latest">
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
      <div className="body-profile-layout">
        {inspectedLine === undefined ? null : (
          <Card>
            <CardHeader>
              <p className="sr-only">Observed values</p>
              <label htmlFor={selectId} className="sr-only">
                Exercise to inspect
              </label>
              <div className="progress-inspect">
                <NativeSelect
                  id={selectId}
                  className="w-full"
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
                <p className="workout-muted">
                  Not estimated:{" "}
                  {inspectedLine.omitted_dates.map(shortDay).join(", ")}
                  <span className="sr-only">
                    . No valid value for this measure; these days have no dot.
                  </span>
                </p>
              ) : null}
              <table className="body-history-table">
                <thead>
                  <tr>
                    <th scope="col">Day</th>
                    <th scope="col">Value</th>
                    <th scope="col">Index</th>
                  </tr>
                </thead>
                <tbody>
                  {inspectedLine.samples.map((sample) => (
                    <tr key={sample.date}>
                      <th scope="row">
                        {shortDay(sample.date)}
                        {sample.date === inspectedLine.baseline.date
                          ? " · baseline"
                          : ""}
                      </th>
                      <td>{rawValue(inspectedLine, sample.value)}</td>
                      <td>{indexValue(sample.index)}</td>
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
              <h2>Not charted</h2>
            </CardHeader>
            <CardContent>
              <ul className="progress-missing">
                {missing.map((line) => (
                  <li key={line.exercise.id}>
                    <div>
                      <span>{line.exercise.name}</span>
                      <span className="workout-muted">
                        {missingLineReason(line)}
                      </span>
                    </div>
                    {removable ? (
                      <Button
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

type ProgressChartProps = {
  readonly from: string
  readonly to: string
  readonly lines: ReadonlyArray<ChartedLine>
  readonly markers: ReadonlyMap<number, Marker>
}

/** Observed days as dots joined in order; long gaps are dashed and nothing is interpolated. */
function ProgressChart({
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
    <figure className="progress-chart" ref={figure}>
      <svg
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
              className="progress-band"
              data-dark={position % 2 === 0}
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
            data-baseline={tick === 100}
            x={plot.left - 8}
            y={scale.y(tick) + 4}
            textAnchor="end"
          >
            {tick}
          </text>
        ))}
        <text x={plot.left} y={size.height - 6}>
          {shortDay(from)}
        </text>
        <text x={right} y={size.height - 6} textAnchor="end">
          {shortDay(to)}
        </text>
        {lines.map((line) => (
          <g
            key={line.exercise.id}
            className="progress-line"
            data-colour={markers.get(line.exercise.id)?.colour}
          >
            {line.samples.slice(1).map((sample, position) => {
              const previous = line.samples[position]

              if (previous === undefined) return null

              return (
                <line
                  key={sample.date}
                  x1={scale.x(previous.date)}
                  y1={scale.y(previous.index)}
                  x2={scale.x(sample.date)}
                  y2={scale.y(sample.index)}
                  data-gap={
                    isGap(previous.date, sample.date) ||
                    line.omitted_dates.some(
                      (date) => date > previous.date && date < sample.date
                    )
                  }
                />
              )
            })}
            {line.samples.map((sample) =>
              markers.get(line.exercise.id)?.dot === "square" ? (
                <rect
                  key={sample.date}
                  x={scale.x(sample.date) - 4}
                  y={scale.y(sample.index) - 4}
                  width={8}
                  height={8}
                />
              ) : (
                <circle
                  key={sample.date}
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
