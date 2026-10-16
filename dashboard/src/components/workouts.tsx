import { useRef, useState } from "react"
import { Link } from "@tanstack/react-router"
import { ArrowUpRightIcon } from "@phosphor-icons/react/dist/csr/ArrowUpRight"
import { BarbellIcon } from "@phosphor-icons/react/dist/csr/Barbell"
import { BicycleIcon } from "@phosphor-icons/react/dist/csr/Bicycle"
import { HeartbeatIcon } from "@phosphor-icons/react/dist/csr/Heartbeat"
import { PencilSimpleIcon } from "@phosphor-icons/react/dist/csr/PencilSimple"
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
import { Button, buttonVariants } from "@/components/ui/button"
import { Card, CardContent, CardHeader } from "@/components/ui/card"
import { WorkoutEditor } from "@/components/workout-editor"
import { formatWhole } from "@/lib/number-format"
import { createRequestId } from "@/lib/request-id"
import { ringShare, shortDuration } from "@/lib/recovery-score"
import {
  clockTime,
  dayLabel,
  sportKind,
  type SportKind,
} from "@/lib/session-format"
import {
  describeSet,
  draftFromWorkout,
  setFigures,
  workoutFailureMessage,
  type LogSession,
  type Strap,
  type WorkoutCommand,
  type WorkoutDetail,
  type WorkoutMutation,
  type WorkoutState,
} from "@/lib/workout"

type Editing = { readonly workoutId: number }

type WorkoutsProps = {
  readonly data: WorkoutState
  readonly change: (command: WorkoutCommand) => Promise<WorkoutMutation>
  /** Read the Log again from Axum. */
  readonly reload: () => Promise<void>
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
  change,
  reload,
  showOlder,
  lead,
}: WorkoutsProps): React.JSX.Element {
  const [editing, setEditing] = useState<Editing>()
  const [busy, setBusy] = useState(false)
  const locked = useRef(false)
  const [feedback, setFeedback] = useState<{ failed: boolean; text: string }>()

  async function run(
    work: () => Promise<void>,
    failure: string
  ): Promise<void> {
    if (locked.current) return
    locked.current = true
    setBusy(true)
    setFeedback(undefined)

    try {
      await work()
    } catch {
      setFeedback({ failed: true, text: failure })
    } finally {
      locked.current = false
      setBusy(false)
    }
  }

  async function save(command: WorkoutCommand): Promise<boolean> {
    if (locked.current) return false
    locked.current = true
    setBusy(true)
    setFeedback(undefined)
    let saved = false

    try {
      const result = await change(command)

      if (result.state === "failed") {
        setFeedback({
          failed: true,
          text: workoutFailureMessage[result.reason],
        })

        return false
      }

      saved = true
      setEditing(undefined)
      await reload()
      setFeedback({
        failed: false,
        text: "Workout saved to OpenHome.",
      })

      return true
    } catch {
      setFeedback({
        failed: true,
        text: saved
          ? "Saved to OpenHome, but the log could not update. Reopen the page to see the change."
          : workoutFailureMessage.unreachable,
      })

      return saved
    } finally {
      locked.current = false
      setBusy(false)
    }
  }

  const workouts =
    data.state === "available"
      ? data.sessions.flatMap((session) =>
          session.workout === undefined ? [] : [session.workout]
        )
      : []

  // An edit drafts from the freshly read Workout; once it is gone the editor closes.
  const active =
    editing !== undefined &&
    !workouts.some((workout) => workout.id === editing.workoutId)
      ? undefined
      : editing

  // Failures belong beside the control that caused them, not above a long editor.
  const toolbarFeedback =
    feedback?.failed === true && active !== undefined ? undefined : feedback

  function editorFor(current: Editing): React.JSX.Element | null {
    if (data.state !== "available") return null

    const workout = workouts.find((item) => item.id === current.workoutId)

    if (workout === undefined) return null

    return (
      <WorkoutEditor
        key={workout.id}
        exercises={data.exercises}
        disabled={busy}
        failure={feedback?.failed === true ? feedback.text : undefined}
        cancel={() => {
          setEditing(undefined)
          setFeedback(undefined)
        }}
        title="Edit workout"
        submitLabel="Save changes"
        initial={draftFromWorkout(workout, createRequestId)}
        save={(input) =>
          save({ kind: "replace", workout_id: workout.id, ...input })
        }
      />
    )
  }

  return (
    <div className="log workouts" aria-busy={busy}>
      <div className="view-bar">{lead}</div>
      <p
        className="workout-feedback"
        role="status"
        data-failed={toolbarFeedback?.failed}
      >
        {toolbarFeedback?.text ?? (busy ? "Working…" : "")}
      </p>
      {data.state === "unavailable" ? (
        <Card>
          <CardHeader>
            <h2>Workouts unavailable</h2>
          </CardHeader>
          <CardContent>
            <p>
              {data.reason === "unreachable"
                ? "OpenHome is unreachable. Open the page again when the connection returns."
                : workoutFailureMessage[data.reason]}
            </p>
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
                  <Session
                    session={session}
                    disabled={busy}
                    locked={active !== undefined}
                    editor={
                      active !== undefined &&
                      active.workoutId === session.workout?.id
                        ? editorFor(active)
                        : undefined
                    }
                    edit={(workout) => {
                      // Another device may have changed it since this page read it.
                      void run(async () => {
                        await reload()
                        setEditing({ workoutId: workout.id })
                      }, "Could not read the latest version of this workout. Try again when OpenHome returns.")
                    }}
                  />
                </li>
              ))}
            </ol>
          )}
          {data.hasOlder ? (
            <div className="log-older">
              <Button
                variant="outline"
                size="lg"
                disabled={busy || active !== undefined}
                onClick={() => {
                  void run(
                    showOlder,
                    "Could not load older workouts. Try again when OpenHome returns."
                  )
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

type SessionProps = {
  readonly session: LogSession
  readonly disabled: boolean
  /** Another session is being edited; this one's actions wait. */
  readonly locked: boolean
  readonly editor: React.JSX.Element | null | undefined
  readonly edit: (workout: WorkoutDetail) => void
}

/**
 * One session on one plate. The header names it; beneath, the logged exercises and the strap's
 * recording sit side by side in the same type, or the strap's map when it is a route.
 */
function Session({
  session,
  disabled,
  locked,
  editor,
  edit,
}: SessionProps): React.JSX.Element {
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
        <div className="session-actions">
          {strap === undefined ? null : (
            <Link
              to="/recovery"
              search={{
                day: session.day,
                noop: {
                  source: strap.summary.source,
                  start: strap.summary.start,
                  sport: strap.summary.sport,
                },
              }}
              className={buttonVariants({
                variant: "ghost",
                size: "key-icon",
              })}
              title="Open in Recovery"
            >
              <ArrowUpRightIcon size={20} aria-hidden="true" />
              <span className="sr-only">Open {title} in Recovery</span>
            </Link>
          )}
          {workout === undefined || editor !== undefined ? null : (
            <>
              <Button
                variant="ghost"
                size="key-icon"
                title="Edit"
                disabled={disabled || locked}
                onClick={() => edit(workout)}
              >
                <PencilSimpleIcon size={20} aria-hidden="true" />
                <span className="sr-only">Edit {title}</span>
              </Button>
            </>
          )}
        </div>
      </header>
      {editor === undefined ? (
        <div className="session-body">
          {workout === undefined ? null : <Exercises workout={workout} />}
          {strap === undefined ? null : (
            <StrapPane strap={strap} route={route} />
          )}
        </div>
      ) : (
        editor
      )}
    </article>
  )
}

/** The logged half: its figures in the strap's style, then each exercise with its sets as pills. */
function Exercises({
  workout,
}: {
  readonly workout: WorkoutDetail
}): React.JSX.Element {
  const sets = workout.exercises.reduce(
    (total, entry) => total + entry.sets.length,
    0
  )

  return (
    <div className="session-logged">
      <StatList
        className="session-stats"
        stats={[
          ["Exercises", String(workout.exercises.length), ""],
          ["Sets", String(sets), ""],
        ]}
      />
      {workout.notes === null ? null : (
        <p className="workout-notes">{workout.notes}</p>
      )}
      {workout.exercises.length === 0 ? (
        <p className="workout-muted">No exercises were logged.</p>
      ) : null}
      <ol className="session-exercises" aria-label="Exercises">
        {workout.exercises.map((entry) => (
          <li key={entry.id}>
            <h4>
              {entry.exercise.name}
              <span className="sr-only"> ({entry.exercise.category})</span>
            </h4>
            {entry.notes === null ? null : (
              <p className="workout-muted">{entry.notes}</p>
            )}
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
                      {set.notes === null ? null : (
                        <span className="set-pill-note">{set.notes}</span>
                      )}
                    </li>
                  )
                })}
              </ol>
            )}
          </li>
        ))}
      </ol>
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
