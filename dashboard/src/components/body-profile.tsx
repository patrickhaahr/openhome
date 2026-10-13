import { useId, useRef, useState } from "react"
import { z } from "zod"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import {
  bodyFailureMessage,
  optionalNumber,
  profileDraft,
  profilePatch,
  type BodyCommand,
  type BodyMutation,
  type BodyRange,
  type BodyState,
  type Profile,
} from "@/lib/body-profile"
import { trainingDayOn } from "@/lib/workout"

type BodyProfileProps = {
  readonly data: BodyState
  readonly range: BodyRange
  readonly change: (command: BodyCommand) => Promise<BodyMutation>
  readonly refresh: () => Promise<void>
  readonly filter: (range: BodyRange) => Promise<void>
}

type Feedback = {
  readonly failed: boolean
  readonly text: string
  readonly area: "profile" | "weight" | "history"
}

const numberFormat = new Intl.NumberFormat("da-DK", {
  maximumFractionDigits: 3,
  useGrouping: false,
})

/** Shared Profile editing and daily Body Weight logging, with no derived health values. */
export function BodyProfile({
  data,
  range,
  change,
  refresh,
  filter,
}: BodyProfileProps): React.JSX.Element {
  const [editing, setEditing] = useState(false)
  const [busy, setBusy] = useState(false)
  const locked = useRef(false)
  const [feedback, setFeedback] = useState<Feedback>()

  async function save(command: BodyCommand): Promise<boolean> {
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
          area: command.kind,
          text: bodyFailureMessage[result.reason],
        })

        return false
      }

      saved = true

      if (command.kind === "profile") setEditing(false)
      await refresh()
      setFeedback({
        failed: false,
        area: command.kind,
        text:
          command.kind === "profile"
            ? "Profile saved to OpenHome."
            : "Body weight saved to OpenHome. Check the date range if it is outside this history.",
      })
    } catch {
      setFeedback({
        failed: true,
        area: command.kind,
        text: saved
          ? "Saved to OpenHome, but the view could not refresh. Refresh to see the change."
          : bodyFailureMessage.unreachable,
      })
    } finally {
      locked.current = false
      setBusy(false)
    }

    return saved
  }

  async function read(work: () => Promise<void>): Promise<void> {
    if (locked.current) return
    locked.current = true
    setBusy(true)
    setFeedback(undefined)

    try {
      await work()
    } catch {
      setFeedback({
        failed: true,
        area: "history",
        text: "Could not refresh. Try again when OpenHome returns.",
      })
    } finally {
      locked.current = false
      setBusy(false)
    }
  }

  return (
    <div className="body-profile workouts" aria-busy={busy}>
      <div className="workout-toolbar">
        <p className="workout-muted">
          Shared fitness data · one profile, one weight record per date
        </p>
        <Button
          variant="outline"
          disabled={busy || editing}
          onClick={() => {
            void read(refresh)
          }}
        >
          Refresh profile & weight
        </Button>
      </div>
      {data.state === "unavailable" ? (
        <Card>
          <CardHeader>
            <h2>Profile & weight unavailable</h2>
          </CardHeader>
          <CardContent>
            <p>
              {data.reason === "unreachable"
                ? "OpenHome is unreachable. Refresh when the connection returns."
                : bodyFailureMessage[data.reason]}
            </p>
          </CardContent>
        </Card>
      ) : (
        <>
          <div className="body-profile-layout">
            <Card>
              <CardHeader>
                <div className="workout-detail-header">
                  <div>
                    <p className="page-eyebrow">Training configuration</p>
                    <h2>Profile</h2>
                  </div>
                  {editing ? null : (
                    <Button
                      variant="outline"
                      disabled={busy}
                      onClick={() => {
                        setFeedback(undefined)
                        setEditing(true)
                      }}
                    >
                      Edit profile
                    </Button>
                  )}
                </div>
              </CardHeader>
              <CardContent>
                {editing ? (
                  <ProfileEditor
                    initial={data.profile}
                    disabled={busy}
                    save={save}
                    failure={
                      feedback?.area === "profile" && feedback.failed
                        ? feedback.text
                        : undefined
                    }
                    cancel={() => {
                      setEditing(false)
                      setFeedback(undefined)
                    }}
                  />
                ) : (
                  <>
                    <dl className="body-profile-values">
                      <div>
                        <dt>Height</dt>
                        <dd>
                          {data.profile.height_cm === null
                            ? "Not configured"
                            : `${numberFormat.format(data.profile.height_cm)} cm`}
                        </dd>
                      </div>
                      <div>
                        <dt>Sex</dt>
                        <dd>{data.profile.sex ?? "Not configured"}</dd>
                      </div>
                      <div>
                        <dt>Maximum heart rate</dt>
                        <dd>
                          {data.profile.hr_max_bpm === null
                            ? "Not configured"
                            : `${data.profile.hr_max_bpm} bpm`}
                        </dd>
                      </div>
                    </dl>
                    <p className="workout-muted">
                      Maximum heart rate defines the NOOP heart rate zones. It
                      is set by you, never inferred from recorded workouts.
                    </p>
                    <FeedbackMessage
                      feedback={
                        feedback?.area === "profile" ? feedback : undefined
                      }
                    />
                  </>
                )}
              </CardContent>
            </Card>
            <Card>
              <CardHeader>
                <p className="page-eyebrow">Daily record</p>
                <h2>Log body weight</h2>
              </CardHeader>
              <CardContent>
                <WeightEditor disabled={busy} save={save} />
                <FeedbackMessage
                  feedback={feedback?.area === "weight" ? feedback : undefined}
                />
                <p className="workout-muted">
                  Each date can be recorded once. Existing entries cannot be
                  edited or replaced.
                </p>
              </CardContent>
            </Card>
          </div>
          <Card>
            <CardHeader>
              <p className="page-eyebrow">Recorded values · kg</p>
              <h2>Body-weight history</h2>
            </CardHeader>
            <CardContent>
              <HistoryFilter
                key={`${range.from ?? ""}/${range.to ?? ""}`}
                initial={range}
                disabled={busy || editing}
                filter={(next) => read(() => filter(next))}
              />
              <p className="workout-muted">
                Oldest first. Up to 500 records per date range; leave either
                date empty for an open range.
              </p>
              {data.history.length === 500 ? (
                <p className="body-history-limit" role="status">
                  This view reached the 500-record limit. Narrow the date range
                  to review later records.
                </p>
              ) : null}
              {data.history.length === 0 ? (
                <p className="body-history-empty">
                  No body weight recorded in this date range.
                </p>
              ) : (
                <table className="body-history-table">
                  <caption className="sr-only">
                    Recorded body weight, oldest first
                  </caption>
                  <thead>
                    <tr>
                      <th scope="col">Date</th>
                      <th scope="col">Weight (kg)</th>
                    </tr>
                  </thead>
                  <tbody>
                    {data.history.map((entry) => (
                      <tr key={entry.id}>
                        <th scope="row">
                          <time dateTime={entry.date}>{entry.date}</time>
                        </th>
                        <td>{numberFormat.format(entry.weight_kg)} kg</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </CardContent>
          </Card>
        </>
      )}
      <FeedbackMessage
        feedback={feedback?.area === "history" ? feedback : undefined}
      />
    </div>
  )
}

function FeedbackMessage({
  feedback,
}: {
  readonly feedback: Feedback | undefined
}): React.JSX.Element {
  return (
    <p
      className="workout-feedback"
      role={feedback?.failed === true ? "alert" : "status"}
      data-failed={feedback?.failed}
    >
      {feedback?.text ?? ""}
    </p>
  )
}

type ProfileEditorProps = {
  readonly initial: Profile
  readonly disabled: boolean
  readonly save: (command: BodyCommand) => Promise<boolean>
  readonly cancel: () => void
  readonly failure: string | undefined
}

function ProfileEditor({
  initial,
  disabled,
  save,
  cancel,
  failure,
}: ProfileEditorProps): React.JSX.Element {
  const [original] = useState(() => profileDraft(initial))
  const [draft, setDraft] = useState(original)

  const [problem, setProblem] = useState<{
    field: "height" | "heart"
    message: string
  }>()

  const id = useId()

  return (
    <form
      className="workout-editor"
      aria-label="Edit profile"
      noValidate
      onSubmit={(event) => {
        event.preventDefault()
        const patch = profilePatch(original, draft)

        if (
          patch.height_cm !== undefined &&
          patch.height_cm !== null &&
          (!Number.isFinite(patch.height_cm) || patch.height_cm <= 0)
        ) {
          setProblem({
            field: "height",
            message:
              "Enter height in cm above zero, or leave it empty to clear.",
          })
          document.getElementById(`${id}-height`)?.focus()

          return
        }

        if (
          patch.hr_max_bpm !== undefined &&
          patch.hr_max_bpm !== null &&
          (!Number.isInteger(patch.hr_max_bpm) ||
            patch.hr_max_bpm < 100 ||
            patch.hr_max_bpm > 240)
        ) {
          setProblem({
            field: "heart",
            message:
              "Enter a whole maximum heart rate from 100 to 240 bpm, or leave it empty to clear.",
          })
          document.getElementById(`${id}-heart`)?.focus()

          return
        }

        setProblem(undefined)
        void save(patch)
      }}
    >
      <p className="workout-muted">
        Leave a field empty to clear it. Unchanged fields keep their stored
        values.
      </p>
      <div className="workout-field">
        <label htmlFor={`${id}-height`}>Height (cm)</label>
        <Input
          id={`${id}-height`}
          aria-invalid={problem?.field === "height"}
          aria-describedby={
            problem?.field === "height" ? `${id}-problem` : undefined
          }
          inputMode="decimal"
          value={draft.height}
          disabled={disabled}
          onChange={(event) => {
            setDraft({ ...draft, height: event.target.value })
            setProblem(undefined)
          }}
        />
      </div>
      <div className="workout-field">
        <label htmlFor={`${id}-sex`}>Sex (optional)</label>
        <Input
          id={`${id}-sex`}
          value={draft.sex}
          disabled={disabled}
          onChange={(event) => setDraft({ ...draft, sex: event.target.value })}
        />
      </div>
      <div className="workout-field">
        <label htmlFor={`${id}-heart`}>Maximum heart rate (bpm)</label>
        <Input
          id={`${id}-heart`}
          aria-invalid={problem?.field === "heart"}
          aria-describedby={
            problem?.field === "heart" ? `${id}-problem` : undefined
          }
          inputMode="numeric"
          value={draft.heartRate}
          disabled={disabled}
          onChange={(event) => {
            setDraft({ ...draft, heartRate: event.target.value })
            setProblem(undefined)
          }}
        />
      </div>
      {problem !== undefined || failure !== undefined ? (
        <p id={`${id}-problem`} className="workout-problem" role="alert">
          {problem?.message ?? failure}
        </p>
      ) : null}
      <div className="workout-editor-actions">
        <Button type="submit" disabled={disabled}>
          Save profile
        </Button>
        <Button variant="outline" disabled={disabled} onClick={cancel}>
          Cancel profile edit
        </Button>
      </div>
    </form>
  )
}

function WeightEditor({
  disabled,
  save,
}: {
  readonly disabled: boolean
  readonly save: (command: BodyCommand) => Promise<boolean>
}): React.JSX.Element {
  const [date, setDate] = useState(() => trainingDayOn(new Date()))
  const [weight, setWeight] = useState("")

  const [problem, setProblem] = useState<{
    field: "date" | "weight"
    message: string
  }>()

  const id = useId()

  return (
    <form
      className="workout-editor"
      aria-label="Log body weight"
      noValidate
      onSubmit={(event) => {
        event.preventDefault()

        if (!z.iso.date().safeParse(date).success) {
          setProblem({
            field: "date",
            message: "Choose a valid calendar date.",
          })
          document.getElementById(`${id}-date`)?.focus()

          return
        }

        const value = optionalNumber(weight)

        if (value === null || !Number.isFinite(value) || value <= 0) {
          setProblem({
            field: "weight",
            message: "Enter body weight in kg above zero.",
          })
          document.getElementById(`${id}-weight`)?.focus()

          return
        }

        setProblem(undefined)
        void save({ kind: "weight", date, weight_kg: value }).then((saved) => {
          if (saved) setWeight("")
        })
      }}
    >
      <div className="workout-editor-fields">
        <div className="workout-field">
          <label htmlFor={`${id}-date`}>Weight date</label>
          <Input
            id={`${id}-date`}
            aria-invalid={problem?.field === "date"}
            aria-describedby={
              problem?.field === "date" ? `${id}-problem` : undefined
            }
            type="date"
            value={date}
            disabled={disabled}
            onChange={(event) => {
              setDate(event.target.value)
              setProblem(undefined)
            }}
          />
        </div>
        <div className="workout-field">
          <label htmlFor={`${id}-weight`}>Body weight (kg)</label>
          <Input
            id={`${id}-weight`}
            aria-invalid={problem?.field === "weight"}
            aria-describedby={
              problem?.field === "weight" ? `${id}-problem` : undefined
            }
            inputMode="decimal"
            value={weight}
            disabled={disabled}
            onChange={(event) => {
              setWeight(event.target.value)
              setProblem(undefined)
            }}
          />
        </div>
      </div>
      {problem === undefined ? null : (
        <p id={`${id}-problem`} className="workout-problem" role="alert">
          {problem.message}
        </p>
      )}
      <div>
        <Button type="submit" disabled={disabled}>
          Save body weight
        </Button>
      </div>
    </form>
  )
}

function HistoryFilter({
  initial,
  disabled,
  filter,
}: {
  readonly initial: BodyRange
  readonly disabled: boolean
  readonly filter: (range: BodyRange) => Promise<void>
}): React.JSX.Element {
  const [from, setFrom] = useState(initial.from ?? "")
  const [to, setTo] = useState(initial.to ?? "")
  const [problem, setProblem] = useState<string>()
  const id = useId()

  return (
    <form
      className="body-history-filter"
      aria-label="Filter body-weight history"
      noValidate
      onSubmit={(event) => {
        event.preventDefault()

        if (
          (from !== "" && !z.iso.date().safeParse(from).success) ||
          (to !== "" && !z.iso.date().safeParse(to).success) ||
          (from !== "" && to !== "" && from > to)
        ) {
          setProblem(
            "Choose a valid date range with the start on or before the end."
          )
          document.getElementById(`${id}-from`)?.focus()

          return
        }

        setProblem(undefined)
        void filter({
          from: from === "" ? undefined : from,
          to: to === "" ? undefined : to,
        })
      }}
    >
      <div className="workout-field">
        <label htmlFor={`${id}-from`}>History from</label>
        <Input
          id={`${id}-from`}
          aria-invalid={problem !== undefined}
          aria-describedby={problem === undefined ? undefined : `${id}-problem`}
          type="date"
          disabled={disabled}
          value={from}
          onChange={(event) => setFrom(event.target.value)}
        />
      </div>
      <div className="workout-field">
        <label htmlFor={`${id}-to`}>History to</label>
        <Input
          id={`${id}-to`}
          aria-invalid={problem !== undefined}
          aria-describedby={problem === undefined ? undefined : `${id}-problem`}
          type="date"
          disabled={disabled}
          value={to}
          onChange={(event) => setTo(event.target.value)}
        />
      </div>
      <Button variant="outline" type="submit" disabled={disabled}>
        Apply date range
      </Button>
      {problem === undefined ? null : (
        <p id={`${id}-problem`} className="workout-problem" role="alert">
          {problem}
        </p>
      )}
    </form>
  )
}
