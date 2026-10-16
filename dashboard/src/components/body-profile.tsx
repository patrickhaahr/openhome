import { useId, useRef, useState } from "react"
import { z } from "zod"
import { CheckIcon } from "@phosphor-icons/react/dist/csr/Check"
import { GenderIntersexIcon } from "@phosphor-icons/react/dist/csr/GenderIntersex"
import { HeartbeatIcon } from "@phosphor-icons/react/dist/csr/Heartbeat"
import { PencilSimpleIcon } from "@phosphor-icons/react/dist/csr/PencilSimple"
import { RulerIcon } from "@phosphor-icons/react/dist/csr/Ruler"
import { ScalesIcon } from "@phosphor-icons/react/dist/csr/Scales"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { WeightChart } from "@/components/weight-chart"
import {
  bodyFailureMessage,
  optionalNumber,
  profileDraft,
  profilePatch,
  type BodyCommand,
  type BodyMutation,
  type BodyRange,
  type BodyState,
  type BodyWeight,
} from "@/lib/body-profile"
import { shiftDay } from "@/lib/training"
import { trainingDayOn } from "@/lib/workout"

type BodyProfileProps = {
  readonly data: BodyState
  readonly range: BodyRange
  readonly change: (command: BodyCommand) => Promise<BodyMutation>
  /** Read the view again after a write. */
  readonly reload: () => Promise<void>
  /** The view switcher, which sits at the start of the toolbar. */
  readonly lead?: React.ReactNode
  readonly filter: (range: BodyRange) => Promise<void>
}

type Feedback = {
  readonly failed: boolean
  readonly text: string
  readonly area: "profile" | "weight" | "history"
}

type ProfileEditing = {
  readonly original: ReturnType<typeof profileDraft>
  readonly draft: ReturnType<typeof profileDraft>
}

const numberFormat = new Intl.NumberFormat("en-GB", {
  maximumFractionDigits: 3,
  useGrouping: false,
})

const oneDecimal = new Intl.NumberFormat("en-GB", {
  minimumFractionDigits: 1,
  maximumFractionDigits: 1,
})

/** Shared Profile editing and daily Body Weight logging, with no derived health values. */
export function BodyProfile({
  data,
  range,
  change,
  reload,
  lead,
  filter,
}: BodyProfileProps): React.JSX.Element {
  // Keep the draft above the read-dependent view so an outage cannot discard it.
  const [editing, setEditing] = useState<ProfileEditing>()
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

      if (command.kind === "profile") setEditing(undefined)
      await reload()
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
          ? "Saved to OpenHome, but the view could not update. Open Body again to see the change."
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
        text: "Could not update. Try again when OpenHome returns.",
      })
    } finally {
      locked.current = false
      setBusy(false)
    }
  }

  const [today] = useState(() => trainingDayOn(new Date()))

  const ranges = [
    { label: "1M", from: shiftDay(today, -30) },
    { label: "3M", from: shiftDay(today, -91) },
    { label: "1Y", from: shiftDay(today, -365) },
    { label: "All", from: undefined },
  ] as const

  const custom =
    range.to !== undefined ||
    !ranges.some((option) => option.from === range.from)

  return (
    <div className="body" aria-busy={busy}>
      <p className="sr-only">
        Shared fitness data · one profile, one weight record per date
      </p>
      <div className="view-bar">
        {lead}
        <div className="view-bar-tools">
          {data.state === "unavailable" ? null : (
            <div
              className="body-ranges"
              role="group"
              aria-label="Body-weight history range"
            >
              {ranges.map((option) => (
                <button
                  key={option.label}
                  type="button"
                  className="body-range"
                  aria-pressed={!custom && option.from === range.from}
                  disabled={busy || editing !== undefined}
                  onClick={() =>
                    void read(() =>
                      filter({ from: option.from, to: undefined })
                    )
                  }
                >
                  {option.label}
                </button>
              ))}
            </div>
          )}
        </div>
      </div>
      {data.state === "unavailable" ? (
        <div className="today-failure">
          <h2>Profile & weight unavailable</h2>
          <p>
            {data.reason === "unreachable"
              ? "OpenHome is unreachable. Open Body again when the connection returns."
              : bodyFailureMessage[data.reason]}
          </p>
        </div>
      ) : (
        <div className="body-layout">
          <div className="body-trend">
            <h2 className="sr-only">Body-weight history</h2>
            <WeightFigure history={data.history} />
            {data.history.length === 500 ? (
              <p className="body-history-limit" role="status">
                This view reached the 500-record limit. Choose a shorter range
                to review later records.
              </p>
            ) : null}
            {data.history.length === 0 ? (
              <p className="body-history-empty">
                No body weight recorded in this date range.
              </p>
            ) : (
              <>
                <WeightChart history={data.history} />
                <div className="sr-only">
                  <table>
                    <caption>Recorded body weight, oldest first</caption>
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
                </div>
              </>
            )}
          </div>
          <div className="body-side">
            <section className="body-plate" aria-labelledby="body-log-title">
              <h2 id="body-log-title" className="sr-only">
                Log body weight
              </h2>
              <WeightEditor disabled={busy} save={save} />
              <FeedbackMessage
                feedback={feedback?.area === "weight" ? feedback : undefined}
              />
              <p className="sr-only">
                Each date can be recorded once. Existing entries cannot be
                edited or replaced.
              </p>
            </section>
            <section
              className="body-profile"
              aria-labelledby="body-profile-title"
            >
              <h2 id="body-profile-title" className="sr-only">
                Profile
              </h2>
              {editing !== undefined ? (
                <div className="body-plate">
                  <ProfileEditor
                    original={editing.original}
                    draft={editing.draft}
                    update={(draft) =>
                      setEditing((current) =>
                        current === undefined
                          ? undefined
                          : { ...current, draft }
                      )
                    }
                    disabled={busy}
                    save={save}
                    failure={
                      feedback?.area === "profile" && feedback.failed
                        ? feedback.text
                        : undefined
                    }
                    cancel={() => {
                      setEditing(undefined)
                      setFeedback(undefined)
                    }}
                  />
                </div>
              ) : (
                <>
                  <dl className="body-profile-values">
                    <div>
                      <dt>
                        <RulerIcon size={20} aria-hidden="true" />
                        <span className="sr-only">Height</span>
                      </dt>
                      <dd data-gap={data.profile.height_cm === null}>
                        {data.profile.height_cm === null
                          ? "Not configured"
                          : numberFormat.format(data.profile.height_cm)}
                        {data.profile.height_cm === null ? null : (
                          <small> cm</small>
                        )}
                      </dd>
                    </div>
                    <div>
                      <dt>
                        <GenderIntersexIcon size={20} aria-hidden="true" />
                        <span className="sr-only">Sex</span>
                      </dt>
                      <dd data-gap={data.profile.sex === null}>
                        {data.profile.sex ?? "Not configured"}
                      </dd>
                    </div>
                    <div>
                      <dt>
                        <HeartbeatIcon size={20} aria-hidden="true" />
                        <span className="sr-only">Maximum heart rate</span>
                      </dt>
                      <dd data-gap={data.profile.hr_max_bpm === null}>
                        {data.profile.hr_max_bpm === null
                          ? "Not configured"
                          : data.profile.hr_max_bpm}
                        {data.profile.hr_max_bpm === null ? null : (
                          <small> max</small>
                        )}
                      </dd>
                    </div>
                  </dl>
                  <div className="body-profile-edit">
                    <Button
                      variant="ghost"
                      size="key-icon"
                      title="Edit profile"
                      disabled={busy}
                      onClick={() => {
                        setFeedback(undefined)
                        const original = profileDraft(data.profile)
                        setEditing({ original, draft: original })
                      }}
                    >
                      <PencilSimpleIcon size={20} aria-hidden="true" />
                      <span className="sr-only">Edit profile</span>
                    </Button>
                  </div>
                  <p className="sr-only">
                    Maximum heart rate defines the NOOP heart rate zones. It is
                    set by you, never inferred from recorded workouts.
                  </p>
                  <FeedbackMessage
                    feedback={
                      feedback?.area === "profile" ? feedback : undefined
                    }
                  />
                </>
              )}
            </section>
          </div>
        </div>
      )}
      <FeedbackMessage
        feedback={
          data.state === "unavailable" || feedback?.area === "history"
            ? feedback
            : undefined
        }
      />
    </div>
  )
}

function directionMark(change: number): string {
  if (change > 0) return "▲"

  return change < 0 ? "▼" : "■"
}

/** The latest weigh-in, large, and how far it moved across the range shown. */
function WeightFigure({
  history,
}: {
  readonly history: ReadonlyArray<BodyWeight>
}): React.JSX.Element | null {
  const first = history[0]
  const last = history.at(-1)

  if (first === undefined || last === undefined) return null

  const change = last.weight_kg - first.weight_kg
  const rounded = Math.round(change * 10) / 10

  return (
    <p className="body-figure">
      <span className="sr-only">Latest body weight, {last.date}: </span>
      <strong>{oneDecimal.format(last.weight_kg)}</strong>
      <small>kg</small>
      {history.length < 2 ? null : (
        <span className="body-change" data-direction={Math.sign(rounded)}>
          <span aria-hidden="true">{directionMark(rounded)}</span>
          <span className="sr-only"> Change since {first.date}: </span>
          {oneDecimal.format(Math.abs(rounded))}
          <small> kg</small>
        </span>
      )}
    </p>
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
  readonly original: ProfileEditing["original"]
  readonly draft: ProfileEditing["draft"]
  readonly update: (draft: ProfileEditing["draft"]) => void
  readonly disabled: boolean
  readonly save: (command: BodyCommand) => Promise<boolean>
  readonly cancel: () => void
  readonly failure: string | undefined
}

function ProfileEditor({
  original,
  draft,
  update,
  disabled,
  save,
  cancel,
  failure,
}: ProfileEditorProps): React.JSX.Element {
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
      <p className="sr-only">
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
            update({ ...draft, height: event.target.value })
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
          onChange={(event) => update({ ...draft, sex: event.target.value })}
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
            update({ ...draft, heartRate: event.target.value })
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
      className="body-weight-form"
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
      <div className="body-weight-fields">
        <div className="body-weight-amount">
          <label htmlFor={`${id}-weight`} className="sr-only">
            Body weight (kg)
          </label>
          <ScalesIcon size={24} aria-hidden="true" />
          <Input
            id={`${id}-weight`}
            aria-invalid={problem?.field === "weight"}
            aria-describedby={
              problem?.field === "weight" ? `${id}-problem` : undefined
            }
            inputMode="decimal"
            placeholder="00.0"
            value={weight}
            disabled={disabled}
            onChange={(event) => {
              setWeight(event.target.value)
              setProblem(undefined)
            }}
          />
          <span aria-hidden="true">kg</span>
        </div>
        <div className="body-weight-date">
          <label htmlFor={`${id}-date`} className="sr-only">
            Weight date
          </label>
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
        <Button type="submit" size="key-icon" disabled={disabled}>
          <CheckIcon size={22} weight="bold" aria-hidden="true" />
          <span className="sr-only">Save body weight</span>
        </Button>
      </div>
      {problem === undefined ? null : (
        <p id={`${id}-problem`} className="workout-problem" role="alert">
          {problem.message}
        </p>
      )}
    </form>
  )
}
