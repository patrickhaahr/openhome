import { useRef, useState } from "react"
import { ArrowClockwiseIcon } from "@phosphor-icons/react/dist/csr/ArrowClockwise"
import { BarbellIcon } from "@phosphor-icons/react/dist/csr/Barbell"
import { PencilSimpleIcon } from "@phosphor-icons/react/dist/csr/PencilSimple"
import { PlusIcon } from "@phosphor-icons/react/dist/csr/Plus"
import { StackIcon } from "@phosphor-icons/react/dist/csr/Stack"
import { TrashIcon } from "@phosphor-icons/react/dist/csr/Trash"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader } from "@/components/ui/card"
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog"
import { WorkoutEditor } from "@/components/workout-editor"
import { createRequestId } from "@/lib/request-id"
import {
  describeSet,
  draftFromWorkout,
  setFigures,
  trainingDayOn,
  workoutFailureMessage,
  type WorkoutCommand,
  type WorkoutDetail,
  type WorkoutMutation,
  type WorkoutState,
} from "@/lib/workout"

const longDate = new Intl.DateTimeFormat("en-GB", {
  timeZone: "UTC",
  weekday: "long",
  day: "numeric",
  month: "long",
  year: "numeric",
})

const shortDate = new Intl.DateTimeFormat("en-GB", {
  timeZone: "UTC",
  weekday: "short",
  day: "numeric",
  month: "short",
})

/** Training Days are calendar dates; formatting them at UTC midnight never shifts the day. */
function formatDay(day: string, format: Intl.DateTimeFormat): string {
  const date = new Date(`${day}T00:00:00Z`)

  return Number.isNaN(date.getTime()) ? day : format.format(date)
}

function entryCount(count: number): string {
  return `${count} ${count === 1 ? "exercise" : "exercises"}`
}

type Editing =
  | { readonly kind: "create"; readonly today: string }
  | { readonly kind: "replace"; readonly workoutId: number }

type WorkoutsProps = {
  readonly data: WorkoutState
  readonly change: (command: WorkoutCommand) => Promise<WorkoutMutation>
  /** Show a Workout (or the newest when undefined) from freshly read history. */
  readonly open: (workoutId: number | undefined) => Promise<void>
  /** Re-read the current view from Axum. */
  readonly refresh: () => Promise<void>
  /** List another page of older Workouts. */
  readonly showOlder: () => Promise<void>
}

/** Logged Workout history and editing, kept apart from NOOP Workouts recorded by the strap. */
export function Workouts({
  data,
  change,
  open,
  refresh,
  showOlder,
}: WorkoutsProps): React.JSX.Element {
  const [editing, setEditing] = useState<Editing>()
  const [removal, setRemoval] = useState<WorkoutDetail>()
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
      setRemoval(undefined)
      await open(result.state === "saved" ? result.workoutId : undefined)
      setFeedback({
        failed: false,
        text:
          result.state === "saved"
            ? "Workout saved to OpenHome."
            : "Workout removed from OpenHome.",
      })

      return true
    } catch {
      setFeedback({
        failed: true,
        text: saved
          ? "Saved to OpenHome, but the view could not refresh. Refresh history to see the change."
          : workoutFailureMessage.unreachable,
      })

      return saved
    } finally {
      locked.current = false
      setBusy(false)
    }
  }

  const selected =
    data.state === "available" && data.selected.state === "found"
      ? data.selected.workout
      : undefined

  // An edit drafts from the freshly read Workout; once it is gone the editor closes.
  const active =
    editing?.kind === "replace" && selected?.id !== editing.workoutId
      ? undefined
      : editing

  // Failures belong beside the control that caused them, not above a long editor.
  const toolbarFeedback =
    feedback?.failed === true && (active !== undefined || removal !== undefined)
      ? undefined
      : feedback

  function editorFor(current: Editing): React.JSX.Element | null {
    if (data.state !== "available") return null

    const shared = {
      exercises: data.exercises,
      disabled: busy,
      failure: feedback?.failed === true ? feedback.text : undefined,
      cancel: () => {
        setEditing(undefined)
        setFeedback(undefined)
      },
    }

    switch (current.kind) {
      case "create":
        return (
          <WorkoutEditor
            {...shared}
            key="new"
            title="Log a workout"
            submitLabel="Save workout"
            initial={{ date: current.today, name: "", notes: "", entries: [] }}
            save={(input) => save({ kind: "create", ...input })}
          />
        )
      case "replace":
        if (selected === undefined) return null

        return (
          <WorkoutEditor
            {...shared}
            key={selected.id}
            title="Edit workout"
            submitLabel="Save changes"
            initial={draftFromWorkout(selected, createRequestId)}
            save={(input) =>
              save({
                kind: "replace",
                workout_id: selected.id,
                ...input,
              })
            }
          />
        )
    }
  }

  return (
    <div className="workouts" aria-busy={busy}>
      <div className="workout-toolbar">
        <p className="sr-only">Logged in OpenHome</p>
        <div className="workout-toolbar-actions">
          <Button
            variant="outline"
            size="key-icon"
            title="Refresh history"
            data-spinning={busy}
            disabled={busy || active !== undefined}
            onClick={() => {
              void run(async () => {
                await refresh()
                setFeedback({ failed: false, text: "History refreshed." })
              }, "Could not refresh. Try again when OpenHome returns.")
            }}
          >
            <ArrowClockwiseIcon size={20} aria-hidden="true" />
            <span className="sr-only">Refresh history</span>
          </Button>
          {data.state === "available" ? (
            <Button
              disabled={busy || active !== undefined}
              onClick={() => {
                setFeedback(undefined)
                setEditing({ kind: "create", today: trainingDayOn(new Date()) })
              }}
            >
              <PlusIcon aria-hidden="true" />
              Log workout
            </Button>
          ) : null}
        </div>
      </div>
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
                ? "OpenHome is unreachable. Refresh history when the connection returns."
                : workoutFailureMessage[data.reason]}
            </p>
          </CardContent>
        </Card>
      ) : (
        <div className="workout-layout">
          <aside className="workout-history" aria-label="Workout history">
            <div className="sr-only">
              <h2>History</h2>
              <p>
                Workouts logged here. NOOP strap workouts are separate records
                and are never matched to them.
              </p>
            </div>
            {data.history.length === 0 ? (
              <p className="workout-muted">No workouts logged yet.</p>
            ) : (
              <ul>
                {data.history.map((workout) => (
                  <li key={workout.id}>
                    <Button
                      variant="ghost"
                      aria-pressed={selected?.id === workout.id}
                      disabled={busy || active !== undefined}
                      onClick={() => {
                        setFeedback(undefined)
                        void run(
                          () => open(workout.id),
                          "Could not open that workout. Refresh history and try again."
                        )
                      }}
                    >
                      <span className="workout-history-day" aria-hidden="true">
                        {Number(workout.date.slice(8, 10))}
                      </span>
                      <span className="workout-history-date">
                        {formatDay(workout.date, shortDate)}
                      </span>
                      <span className="workout-history-name">
                        {workout.name ?? "Workout"}
                      </span>
                    </Button>
                  </li>
                ))}
              </ul>
            )}
            {data.hasOlder ? (
              <Button
                variant="outline"
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
            ) : null}
          </aside>
          <Card>
            <CardContent>
              {active === undefined ? (
                <WorkoutView
                  data={data}
                  disabled={busy}
                  edit={(workout) => {
                    // Another device may have changed it since this page read it.
                    void run(async () => {
                      await open(workout.id)
                      setEditing({ kind: "replace", workoutId: workout.id })
                    }, "Could not read the latest version of this workout. Refresh history and try again.")
                  }}
                  remove={setRemoval}
                />
              ) : (
                editorFor(active)
              )}
            </CardContent>
          </Card>
        </div>
      )}
      <AlertDialog
        open={removal !== undefined}
        onOpenChange={(next) => {
          if (!next && !busy) setRemoval(undefined)
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Remove this workout?</AlertDialogTitle>
            <AlertDialogDescription>
              {removal === undefined
                ? ""
                : `Remove ${removal.name ?? "the workout"} from ${formatDay(removal.date, longDate)} with ${entryCount(removal.exercises.length)}? NOOP records are not affected. This cannot be undone.`}
            </AlertDialogDescription>
          </AlertDialogHeader>
          {feedback?.failed === true ? (
            <p role="alert" className="workout-feedback" data-failed="true">
              {feedback.text}
            </p>
          ) : null}
          <AlertDialogFooter>
            <AlertDialogCancel size="lg" disabled={busy}>
              Keep it
            </AlertDialogCancel>
            <Button
              variant="destructive"
              size="lg"
              disabled={busy}
              onClick={() => {
                if (removal !== undefined)
                  void save({ kind: "remove", workout_id: removal.id })
              }}
            >
              Remove permanently
            </Button>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  )
}

type WorkoutViewProps = {
  readonly data: Extract<WorkoutState, { state: "available" }>
  readonly disabled: boolean
  readonly edit: (workout: WorkoutDetail) => void
  readonly remove: (workout: WorkoutDetail) => void
}

function WorkoutView({
  data,
  disabled,
  edit,
  remove,
}: WorkoutViewProps): React.JSX.Element {
  switch (data.selected.state) {
    case "none":
      return (
        <div className="workout-empty">
          <BarbellIcon size={40} aria-hidden="true" />
          <h2>Ready when you are.</h2>
          <p>Log a workout to keep its exercises and sets here.</p>
        </div>
      )
    case "missing":
      return (
        <div className="workout-empty">
          <BarbellIcon size={40} aria-hidden="true" />
          <h2>Workout not found</h2>
          <p>{workoutFailureMessage["not-found"]}</p>
        </div>
      )
    case "found":
      break
  }

  const workout = data.selected.workout

  return (
    <article aria-labelledby="workout-title">
      <header className="workout-detail-header">
        <div>
          <p className="workout-date">{formatDay(workout.date, longDate)}</p>
          <h2 id="workout-title">{workout.name ?? "Workout"}</h2>
          <p className="workout-tally">
            <span>
              <strong>{workout.exercises.length}</strong>{" "}
              <span className="sr-only">
                {workout.exercises.length === 1 ? "exercise" : "exercises"}
              </span>
              <BarbellIcon size={18} aria-hidden="true" />
            </span>
            <span className="sr-only"> · </span>
            <span>
              <strong>
                {workout.exercises.reduce(
                  (total, entry) => total + entry.sets.length,
                  0
                )}
              </strong>{" "}
              <span className="sr-only">sets</span>
              <StackIcon size={18} aria-hidden="true" />
            </span>
          </p>
        </div>
        <div className="workout-detail-actions">
          <Button
            variant="outline"
            size="key-icon"
            title="Edit"
            disabled={disabled}
            onClick={() => edit(workout)}
          >
            <PencilSimpleIcon size={20} aria-hidden="true" />
            <span className="sr-only">Edit</span>
          </Button>
          <Button
            variant="outline"
            size="key-icon"
            title="Remove"
            disabled={disabled}
            onClick={() => remove(workout)}
          >
            <TrashIcon size={20} aria-hidden="true" />
            <span className="sr-only">Remove</span>
          </Button>
        </div>
      </header>
      {workout.notes === null ? null : (
        <p className="workout-notes">{workout.notes}</p>
      )}
      {workout.exercises.length === 0 ? (
        <p className="workout-muted">No exercises were logged.</p>
      ) : null}
      <ol className="workout-entries" aria-label="Exercises">
        {workout.exercises.map((entry) => (
          <li key={entry.id} className="workout-entry">
            <div className="workout-entry-heading">
              <h3>{entry.exercise.name}</h3>
              <span className="sr-only">{entry.exercise.category}</span>
            </div>
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
          </li>
        ))}
      </ol>
    </article>
  )
}
