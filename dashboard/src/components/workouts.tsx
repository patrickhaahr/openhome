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
    <div className="log workouts" aria-busy={busy}>
      <div className="view-bar">{lead}</div>
      <p
        className="workout-feedback"
        role="status"
        data-failed={failure === undefined ? undefined : true}
      >
        {failure ?? (busy ? "Working…" : "")}
      </p>
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
            <p className="workout-feedback" data-failed="true" role="status">
              The strap is unreachable, so only logged workouts show.
            </p>
          ) : null}
          {data.sessions.length === 0 ? (
            <div className="workout-empty">
              <BarbellIcon size={40} aria-hidden="true" />
              <h2>Ready when you are.</h2>
              <p>Workouts you log appear here with the strap's recording.</p>
            </div>
          ) : (
            <ol className="log-list" aria-label="Sessions, newest first">
              {data.sessions.map((session) => (
                <li key={session.key}>
                  <Session session={session} />
                </li>
              ))}
            </ol>
          )}
          {data.hasOlder ? (
            <div className="log-older">
              <Button
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
      className="session"
      data-kind={layout}
      data-route={route !== undefined}
      aria-labelledby={`session-${session.key}`}
    >
      <header className="session-head">
        <span className="session-glyph" aria-hidden="true">
          <SessionGlyph kind={kind} />
        </span>
        <div className="session-title">
          <h3 id={`session-${session.key}`}>{title}</h3>
          <p>
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
      <div className="session-body">
        {workout === undefined ? null : <Exercises workout={workout} />}
        {strap === undefined ? null : <StrapPane strap={strap} route={route} />}
      </div>
    </article>
  )
}

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
    <dl className="session-counts">
      <div>
        <dt className="sr-only">Exercises</dt>
        <dd>
          <span className="num">{workout.exercises.length}</span>
          <small aria-hidden="true">
            {workout.exercises.length === 1 ? " exercise" : " exercises"}
          </small>
        </dd>
      </div>
      <div>
        <dt className="sr-only">Sets</dt>
        <dd>
          <span className="num">{sets}</span>
          <small aria-hidden="true">{sets === 1 ? " set" : " sets"}</small>
        </dd>
      </div>
    </dl>
  )
}

/**
 * The logged half: one row per exercise on one shared grid, its name first and its sets in
 * columns of equal width, so a set lines up with the same set of every other exercise.
 */
function Exercises({
  workout,
}: {
  readonly workout: WorkoutDetail
}): React.JSX.Element {
  const columns = Math.min(
    maxSetColumns,
    Math.max(1, ...workout.exercises.map((entry) => entry.sets.length))
  )

  return (
    <div className="session-logged">
      {workout.notes === null ? null : (
        <p className="workout-notes">{workout.notes}</p>
      )}
      {workout.exercises.length === 0 ? (
        <p className="workout-muted">No exercises were logged.</p>
      ) : (
        <ol
          className="session-exercises"
          aria-label="Exercises"
          // SAFETY: CSSProperties omits custom properties; --set-columns is the grid's set column count.
          style={{ "--set-columns": columns } as React.CSSProperties}
        >
          {workout.exercises.map((entry) => (
            <li key={entry.id}>
              <h4>
                {entry.exercise.name}
                <span className="sr-only"> ({entry.exercise.category})</span>
              </h4>
              {entry.sets.length === 0 ? (
                <p className="workout-muted">No sets logged.</p>
              ) : (
                <ol className="workout-sets">
                  {entry.sets.map((set, setIndex) => {
                    const figures = setFigures(set)

                    return (
                      <li
                        key={set.id}
                        className="set-pill"
                        data-rpe={set.rpe ?? undefined}
                        title={set.notes ?? undefined}
                      >
                        <span className="sr-only">
                          Set {setIndex + 1}: {describeSet(set)}
                        </span>
                        <span className="set-pill-effort" aria-hidden="true">
                          {figures.effort}
                        </span>
                        <span className="set-pill-load" aria-hidden="true">
                          {figures.load ?? "BW"}
                        </span>
                        {set.rpe === null ? null : (
                          <span
                            className="set-pill-rpe"
                            aria-hidden="true"
                            // SAFETY: CSSProperties omits custom properties; --rpe is a 0..1 heat read by .set-pill-rpe.
                            style={
                              { "--rpe": set.rpe / 10 } as React.CSSProperties
                            }
                          />
                        )}
                        {set.notes === null ? null : (
                          <span className="set-pill-note">{set.notes}</span>
                        )}
                      </li>
                    )
                  })}
                </ol>
              )}
              {entry.notes === null ? null : (
                <p className="session-exercise-note workout-muted">
                  {entry.notes}
                </p>
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
}: {
  readonly strap: Strap
  readonly route: Route | undefined
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
    <div className="session-trace">
      {detail?.hr_series === null || detail === undefined ? null : (
        <HeartRateChart
          series={detail.hr_series}
          zones={detail.hr_zones}
          durationS={
            (Date.parse(summary.end) - Date.parse(summary.start)) / 1000
          }
        />
      )}
      {detail?.hr_zones === null || detail === undefined ? null : (
        <ZoneBar zones={detail.hr_zones} />
      )}
    </div>
  )

  return (
    <>
      <div className="session-strap">
        <div className="session-strap-top">
          <ScoreRing
            kind="effort"
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
          <StatList className="session-stats" stats={stats} />
        </div>
        {trace}
      </div>
      {route === undefined ? null : (
        <div className="session-map">
          <RouteMap route={route} />
        </div>
      )}
    </>
  )
}
