import { useRef, useState } from "react"
import { BarbellIcon } from "@phosphor-icons/react/dist/csr/Barbell"
import { BicycleIcon } from "@phosphor-icons/react/dist/csr/Bicycle"
import { HeartbeatIcon } from "@phosphor-icons/react/dist/csr/Heartbeat"
import { PersonSimpleRunIcon } from "@phosphor-icons/react/dist/csr/PersonSimpleRun"
import { PersonSimpleWalkIcon } from "@phosphor-icons/react/dist/csr/PersonSimpleWalk"
import {
  HeartRateChart,
  noopStats,
  RouteMap,
  StatList,
  ZoneBar,
} from "@/components/noop-detail"
import { ScoreRing } from "@/components/score-ring"
import {
  ViewBar,
  WorkoutFeedback,
  workoutMuted,
} from "@/components/workout-parts"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader } from "@/components/ui/card"
import { formatWhole } from "@/lib/number-format"
import { ringShare, shortDuration } from "@/lib/recovery-score"
import {
  clockTime,
  dayLabel,
  sportKind,
  type SportKind,
} from "@/lib/session-format"
import {
  describeSet,
  setFigures,
  workoutFailureMessage,
  type LogSession,
  type Strap,
  type WorkoutDetail,
  type WorkoutState,
} from "@/lib/workout"

type WorkoutsProps = {
  readonly data: WorkoutState
  /** List another page of older Workouts. */
  readonly showOlder: () => Promise<void>
  /** The view switcher, which sits at the start of the toolbar. */
  readonly lead?: React.ReactNode
}

/**
 * The training Log: one plate per session, newest first. A logged Workout and the strap's
 * recording of the same session share a plate; the rest stand alone.
 */
export function Workouts({
  data,
  showOlder,
  lead,
}: WorkoutsProps): React.JSX.Element {
  const [busy, setBusy] = useState(false)
  const locked = useRef(false)
  const [failure, setFailure] = useState<string>()

  async function older(): Promise<void> {
    if (locked.current) return
    locked.current = true
    setBusy(true)
    setFailure(undefined)

    try {
      await showOlder()
    } catch {
      setFailure(
        "Could not load older workouts. Try again when OpenHome returns."
      )
    } finally {
      locked.current = false
      setBusy(false)
    }
  }

  return (
    <div className="grid" aria-busy={busy}>
      <ViewBar lead={lead} />
      <WorkoutFeedback failed={failure !== undefined}>
        {failure ?? (busy ? "Working…" : "")}
      </WorkoutFeedback>
      {data.state === "unavailable" ? (
        <Card>
          <CardHeader>
            <h2>Workouts unavailable</h2>
          </CardHeader>
          <CardContent>
            <p>{workoutFailureMessage[data.reason]}</p>
          </CardContent>
        </Card>
      ) : (
        <>
          <h2 className="sr-only">Sessions</h2>
          {data.strap === "unavailable" ? (
            <WorkoutFeedback failed>
              The strap is unreachable, so only logged workouts show.
            </WorkoutFeedback>
          ) : null}
          {data.sessions.length === 0 ? (
            <div className="grid justify-items-start gap-3 py-10">
              <BarbellIcon
                className="text-muted-ink"
                size={40}
                aria-hidden="true"
              />
              <h2>Ready when you are.</h2>
              <p className="text-muted-ink">
                Workouts you log appear here with the strap's recording.
              </p>
            </div>
          ) : (
            <ol className="grid gap-3" aria-label="Sessions, newest first">
              {data.sessions.map((session) => (
                <li key={session.key}>
                  <Session session={session} />
                </li>
              ))}
            </ol>
          )}
          {data.hasOlder ? (
            <div className="mt-3 justify-self-center">
              <Button
                className="touch-manipulation"
                variant="outline"
                size="lg"
                disabled={busy}
                onClick={() => {
                  void older()
                }}
              >
                Show older workouts
              </Button>
            </div>
          ) : null}
        </>
      )}
    </div>
  )
}

function SessionGlyph({
  kind,
}: {
  readonly kind: SportKind | "logged"
}): React.JSX.Element {
  switch (kind) {
    case "run":
      return <PersonSimpleRunIcon size={28} aria-hidden="true" />
    case "walk":
      return <PersonSimpleWalkIcon size={28} aria-hidden="true" />
    case "ride":
      return <BicycleIcon size={28} aria-hidden="true" />
    case "strength":
    case "logged":
      return <BarbellIcon size={28} aria-hidden="true" />
    case "other":
      return <HeartbeatIcon size={28} aria-hidden="true" />
  }
}

/** The widest set row the grid draws; longer entries continue in the same columns beneath. */
const maxSetColumns = 4

/**
 * One session on one plate. The header names it with its counts; beneath, the logged exercises
 * and the strap's recording sit side by side, or the strap's map when it is a route.
 */
function Session({
  session,
}: {
  readonly session: LogSession
}): React.JSX.Element {
  const { workout, strap } = session

  const kind: SportKind | "logged" =
    workout !== undefined || strap === undefined
      ? "logged"
      : sportKind(strap.summary.sport)

  const title = workout?.name ?? strap?.summary.sport ?? "Workout"

  const route =
    workout === undefined &&
    strap?.detail?.route !== undefined &&
    strap.detail.route.length > 1
      ? strap.detail.route
      : undefined

  const layout =
    workout !== undefined && strap !== undefined
      ? "paired"
      : workout === undefined
        ? "strap"
        : "logged"

  return (
    <article
      data-slot="session"
      className="grid min-w-0 gap-5 rounded-panel bg-tile p-session"
      data-kind={layout}
      aria-labelledby={`session-${session.key}`}
    >
      <header className="flex items-center gap-4 max-[700px]:flex-wrap">
        <span
          className="grid size-12 flex-none place-items-center rounded-full bg-wash-strong"
          aria-hidden="true"
        >
          <SessionGlyph kind={kind} />
        </span>
        <div data-slot="session-title" className="min-w-0 flex-1">
          <h3
            id={`session-${session.key}`}
            className="truncate text-lead leading-workout-heading font-semibold tracking-title"
          >
            {title}
          </h3>
          <p className="mt-0.5 text-muted-ink tabular-nums">
            <time dateTime={session.day}>{dayLabel(session.day)}</time>
            {strap === undefined ? null : (
              <>
                <span aria-hidden="true"> · </span>
                <span className="num">
                  {clockTime(strap.summary.start)}
                  <span aria-hidden="true">–</span>
                  <span className="sr-only"> to </span>
                  {clockTime(strap.summary.end)}
                </span>
                {workout === undefined ? null : (
                  <span className="sr-only"> · {strap.summary.sport}</span>
                )}
              </>
            )}
          </p>
        </div>
        {workout === undefined ? null : <Counts workout={workout} />}
      </header>
      <div
        className={`grid items-stretch gap-workout-gutter ${bodyColumns[layout === "strap" && route !== undefined ? "route" : layout]}`}
      >
        {workout === undefined ? null : (
          <Exercises workout={workout} spread={layout === "logged"} />
        )}
        {strap === undefined ? null : (
          <StrapPane
            strap={strap}
            route={route}
            fit={layout === "strap" && route === undefined ? "alone" : "pane"}
          />
        )}
      </div>
    </article>
  )
}

const bodyColumns = {
  paired: "grid-cols-session max-[860px]:grid-cols-1",
  route: "grid-cols-2 max-[860px]:grid-cols-1",
  strap: "grid-cols-1",
  logged: "grid-cols-1",
} as const

/** How much was logged, as two figures where the header's keys once sat. */
function Counts({
  workout,
}: {
  readonly workout: WorkoutDetail
}): React.JSX.Element {
  const sets = workout.exercises.reduce(
    (total, entry) => total + entry.sets.length,
    0
  )

  return (
    <dl
      data-slot="session-counts"
      className="flex flex-none items-baseline gap-5 max-[700px]:order-3 max-[700px]:w-full max-[700px]:pl-16"
    >
      <div>
        <dt className="sr-only">Exercises</dt>
        <dd className={countFigure}>
          <span className="num">{workout.exercises.length}</span>
          <small aria-hidden="true" className={countUnit}>
            {workout.exercises.length === 1 ? " exercise" : " exercises"}
          </small>
        </dd>
      </div>
      <div>
        <dt className="sr-only">Sets</dt>
        <dd className={countFigure}>
          <span className="num">{sets}</span>
          <small aria-hidden="true" className={countUnit}>
            {sets === 1 ? " set" : " sets"}
          </small>
        </dd>
      </div>
    </dl>
  )
}

const countFigure = "text-figure-sm leading-none font-bold tracking-figure"

const countUnit = "text-control font-semibold tracking-normal text-muted-ink"

/**
 * The logged half: one row per exercise on one shared grid, its name first and its sets in
 * columns of equal width, so a set lines up with the same set of every other exercise.
 */
function Exercises({
  workout,
  spread,
}: {
  readonly workout: WorkoutDetail
  /** Alone on its plate, the rows spread over two columns that read top to bottom. */
  readonly spread: boolean
}): React.JSX.Element {
  const columns = Math.min(
    maxSetColumns,
    Math.max(1, ...workout.exercises.map((entry) => entry.sets.length))
  )

  return (
    <div
      data-slot="session-logged"
      className="grid min-w-0 content-start gap-4"
    >
      {workout.notes === null ? null : (
        <p className="mt-4 whitespace-pre-wrap text-muted-ink">
          {workout.notes}
        </p>
      )}
      {workout.exercises.length === 0 ? (
        <p className={workoutMuted}>No exercises were logged.</p>
      ) : (
        <ol
          data-slot="session-exercises"
          className={
            spread ? "exercise-columns gap-x-workout-gutter" : "grid gap-2"
          }
          aria-label="Exercises"
          // SAFETY: CSSProperties omits custom properties; --set-columns is the grid's set column count.
          style={{ "--set-columns": columns } as React.CSSProperties}
        >
          {workout.exercises.map((entry) => (
            <li
              key={entry.id}
              className={`grid grid-cols-named-sets items-baseline gap-2 max-[560px]:mb-2 max-[560px]:grid-cols-even-sets max-[560px]:gap-y-1.5 ${spread ? "mb-2 break-inside-avoid" : ""}`}
            >
              <h4 className="min-w-0 text-body-lg leading-tight font-semibold wrap-anywhere max-[560px]:col-span-full">
                {entry.exercise.name}
                <span className="sr-only"> ({entry.exercise.category})</span>
              </h4>
              {entry.sets.length === 0 ? (
                <p
                  className={`col-start-2 -col-end-1 max-[560px]:col-span-full ${workoutMuted}`}
                >
                  No sets logged.
                </p>
              ) : (
                <ol className="col-start-2 -col-end-1 grid grid-cols-sets gap-2 max-[560px]:col-span-full max-[560px]:grid-cols-even-sets">
                  {entry.sets.map((set, setIndex) => {
                    const figures = setFigures(set)

                    return (
                      <li
                        key={set.id}
                        data-slot="set-pill"
                        className="relative flex min-w-0 flex-wrap items-baseline gap-x-1.25 overflow-hidden rounded-art bg-ground px-3 pt-2 pb-3 tabular-nums dark:bg-wash-strong"
                        data-rpe={set.rpe ?? undefined}
                        title={set.notes ?? undefined}
                      >
                        <span className="sr-only">
                          Set {setIndex + 1}: {describeSet(set)}
                        </span>
                        <span
                          className="text-set leading-none font-bold tracking-title"
                          aria-hidden="true"
                        >
                          {figures.effort}
                        </span>
                        <span
                          className="text-small font-semibold text-muted-ink"
                          aria-hidden="true"
                        >
                          {figures.load ?? "BW"}
                        </span>
                        {set.rpe === null ? null : (
                          <span
                            data-slot="set-pill-rpe"
                            className="absolute bottom-0 left-0 h-1 w-rpe bg-sun"
                            aria-hidden="true"
                            // SAFETY: CSSProperties omits custom properties; --rpe is a 0..1 heat read by w-rpe.
                            style={
                              { "--rpe": set.rpe / 10 } as React.CSSProperties
                            }
                          />
                        )}
                        {set.notes === null ? null : (
                          <span className="basis-full text-caption text-muted-ink">
                            {set.notes}
                          </span>
                        )}
                      </li>
                    )
                  })}
                </ol>
              )}
              {entry.notes === null ? null : (
                <p className={`col-span-full ${workoutMuted}`}>{entry.notes}</p>
              )}
            </li>
          ))}
        </ol>
      )}
    </div>
  )
}

type Route = NonNullable<NonNullable<Strap["detail"]>["route"]>

/** The strap's half: effort and figures, the heart-rate trace on its zones, and the route if there is one. */
function StrapPane({
  strap,
  route,
  fit,
}: {
  readonly strap: Strap
  readonly route: Route | undefined
  readonly fit: "pane" | "alone"
}): React.JSX.Element {
  const { summary, detail } = strap
  const effort = detail?.strain_score ?? summary.strain_score

  const stats =
    detail === undefined
      ? ([
          [
            "Duration",
            summary.duration_min === null
              ? null
              : shortDuration(summary.duration_min),
            "",
          ],
          [
            "Average HR",
            summary.avg_hr_bpm === null
              ? null
              : String(Math.round(summary.avg_hr_bpm)),
            "bpm",
          ],
          [
            "Steps",
            summary.steps === null ? null : formatWhole(summary.steps),
            "",
          ],
        ] as const)
      : noopStats(detail)

  const trace = (
    <div className="grid min-w-0 grid-rows-fill-hug gap-4">
      {detail?.hr_series === null || detail === undefined ? null : (
        <HeartRateChart
          series={detail.hr_series}
          zones={detail.hr_zones}
          durationS={
            (Date.parse(summary.end) - Date.parse(summary.start)) / 1000
          }
          fit={fit}
        />
      )}
      {detail?.hr_zones === null || detail === undefined ? null : (
        <ZoneBar zones={detail.hr_zones} fit={fit} />
      )}
    </div>
  )

  return (
    <>
      <div
        data-slot="session-strap"
        className={
          fit === "alone"
            ? "grid min-w-0 grid-cols-2 grid-rows-none gap-workout-gutter max-[860px]:grid-cols-1 max-[860px]:gap-4"
            : "grid min-w-0 grid-rows-auto-fill gap-5"
        }
      >
        <div className="flex items-center gap-6 max-[700px]:flex-col max-[700px]:items-start max-[700px]:gap-4">
          <ScoreRing
            kind="effort"
            size="strap"
            share={ringShare(effort, 100)}
            band={effort === null ? "none" : "fair"}
            figure={effort === null ? null : String(Math.round(effort))}
            label="Effort"
            spoken={
              effort === null
                ? "Not available"
                : `strain ${Math.round(effort)} of 100`
            }
          />
          <StatList look="session" stats={stats} />
        </div>
        {trace}
      </div>
      {route === undefined ? null : (
        <div data-slot="session-map">
          <RouteMap route={route} fit="pane" />
        </div>
      )}
    </>
  )
}
