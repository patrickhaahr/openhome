import { useRef, useState } from "react"
import { CakeIcon } from "@phosphor-icons/react/dist/csr/Cake"
import { GenderIntersexIcon } from "@phosphor-icons/react/dist/csr/GenderIntersex"
import { HeartbeatIcon } from "@phosphor-icons/react/dist/csr/Heartbeat"
import { PencilSimpleIcon } from "@phosphor-icons/react/dist/csr/PencilSimple"
import { RulerIcon } from "@phosphor-icons/react/dist/csr/Ruler"
import { useIdleBlock } from "@/components/idle-block"
import { Button } from "@/components/ui/button"
import { WeightChart } from "@/components/weight-chart"
import {
  bodyFailureMessage,
  profileDraft,
  type BodyCommand,
  type BodyMutation,
  type BodyRange,
  type BodyState,
  type BodyWeight,
} from "@/lib/body-profile"
import { shiftDay } from "@/lib/training"
import { trainingDayOn } from "@/lib/workout"
import {
  ProfileEditor,
  WeightEditor,
  type ProfileEditing,
} from "@/components/body-profile-editor"

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

const numberFormat = new Intl.NumberFormat("en-GB", {
  maximumFractionDigits: 3,
  useGrouping: false,
})

const birthDay = new Intl.DateTimeFormat("en-GB", {
  timeZone: "UTC",
  day: "numeric",
  month: "long",
  year: "numeric",
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
  // An open profile edit or a save in flight holds the slideshow off.
  useIdleBlock(editing !== undefined || busy)

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
                    <div>
                      <dt>
                        <CakeIcon size={20} aria-hidden="true" />
                        <span className="sr-only">Birth date</span>
                      </dt>
                      <dd data-gap={data.profile.birth_date === null}>
                        {data.profile.birth_date === null
                          ? "Not configured"
                          : birthDay.format(
                              new Date(`${data.profile.birth_date}T12:00:00Z`)
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
                    set by you, never inferred from recorded workouts. Height,
                    sex and birth date set the Food page's Calorie Target.
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
