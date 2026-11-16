import { useRef, useState } from "react"
import { HeartbeatIcon } from "@phosphor-icons/react/dist/csr/Heartbeat"
import { LightningIcon } from "@phosphor-icons/react/dist/csr/Lightning"
import { MoonIcon } from "@phosphor-icons/react/dist/csr/Moon"
import { XIcon } from "@phosphor-icons/react/dist/csr/X"
import { Button } from "@/components/ui/button"
import { NoopDetail } from "@/components/noop-detail"
import { DayStrip } from "@/components/day-strip"
import { ScoreRing } from "@/components/score-ring"
import { chargeBand, ringShare } from "@/lib/recovery-score"
import {
  trainingFailureMessage,
  type NoopSelection,
  type RecoveryDay,
  type TrainingRead,
  type TrainingSearch,
  type TrainingState,
} from "@/lib/training"
import { noopButtonId } from "@/components/training-shared"
import { Night } from "@/components/training-night"
import { Body } from "@/components/training-body"
import { Sessions } from "@/components/training-sessions"

/** The selected day as rings over its week, beside its night; then its body and its strap sessions. Words stay for AT. */
export function TrainingSummaries({
  data,
  selectDay,
  open,
  refresh,
  loadDays,
}: {
  readonly data: TrainingState
  readonly selectDay: (day: string) => Promise<void>
  readonly open: (selection: TrainingSearch["noop"]) => Promise<void>
  readonly refresh: () => Promise<void>
  readonly loadDays?:
    | ((from: string, to: string) => Promise<TrainingState["week"]>)
    | undefined
}): React.JSX.Element {
  const [busy, setBusy] = useState(false)
  const locked = useRef(false)
  const detailRegion = useRef<HTMLDivElement>(null)
  const detailSelection = useRef<NoopSelection | undefined>(undefined)
  const dayButton = useRef<HTMLButtonElement>(null)
  const [feedback, setFeedback] = useState("")
  const today = data.today

  async function run(work: () => Promise<void>): Promise<void> {
    if (locked.current) return
    locked.current = true
    setBusy(true)
    setFeedback("")

    try {
      await work()
    } catch {
      setFeedback("Could not update this view. Refresh to try again.")
    } finally {
      locked.current = false
      setBusy(false)
    }
  }

  const contextDay =
    data.context.state === "available"
      ? data.context.data.days.find((entry) => entry.day === data.day)
      : undefined

  const selected =
    data.detail.state === "available" ? data.detail.data : undefined

  const recovery =
    data.recovery.state === "available" ? data.recovery.data : undefined

  const covered =
    recovery?.noop.coverage.daily_metrics === "covered" &&
    recovery.noop.coverage.sleep_sessions === "covered"

  const missing = covered ? "Not recorded" : "Not synced or unavailable"

  return (
    <div className="today" aria-busy={busy}>
      <h2 className="sr-only">Recovery day · {data.day}</h2>
      <p
        className="workout-feedback"
        role="status"
        data-failed={!busy && feedback !== "" ? true : undefined}
      >
        {busy ? "Reading training data…" : feedback}
      </p>
      <div className="today-hero">
        <div className="today-glance">
          {data.recovery.state === "available" ? (
            <>
              <Sync data={data.recovery.data.noop} />
              <Rings data={data.recovery.data} missing={missing} />
            </>
          ) : (
            <ReadFailure title="Recovery day" data={data.recovery} />
          )}
          <div className="today-week">
            <DayStrip
              initial={data.week}
              selected={data.day}
              today={today}
              disabled={busy}
              selectedRef={dayButton}
              loadDays={loadDays}
              choose={(day, current) =>
                void run(() => (current ? refresh() : selectDay(day)))
              }
            />
          </div>
        </div>
        {recovery === undefined ? null : (
          <Night
            data={recovery}
            trend={data.trends.sleep_duration}
            missing={missing}
          />
        )}
      </div>
      {recovery === undefined ? null : (
        <Body
          data={recovery}
          week={data.week}
          trends={data.trends}
          missing={missing}
        />
      )}
      <section className="today-sessions" aria-label="Sessions">
        {data.context.state === "unavailable" ? (
          <ReadFailure title="Training context" data={data.context} />
        ) : (
          <Sessions
            day={contextDay}
            selected={selected}
            disabled={busy}
            open={(selection) => {
              detailSelection.current = selection
              void run(async () => {
                await open(selection)
                detailRegion.current?.focus({ preventScroll: true })
                detailRegion.current?.scrollIntoView({
                  block: "start",
                  behavior: "smooth",
                })
              })
            }}
          />
        )}
        <div
          ref={detailRegion}
          tabIndex={-1}
          role="region"
          aria-label="NOOP workout detail"
          className="today-detail"
        >
          {data.detail.state === "none" ? null : (
            <>
              <div className="today-detail-close">
                <Button
                  variant="ghost"
                  size="key-icon"
                  title="Close"
                  disabled={busy}
                  onClick={() => {
                    const selection =
                      data.detail.state === "available"
                        ? data.detail.data
                        : detailSelection.current

                    void run(async () => {
                      await open(undefined)
                      // Wait until run releases the disabled controls before restoring focus.
                      requestAnimationFrame(() => {
                        const button =
                          selection === undefined
                            ? null
                            : document.getElementById(noopButtonId(selection))

                        if (
                          button instanceof HTMLButtonElement &&
                          !button.disabled
                        )
                          button.focus()
                        else dayButton.current?.focus()
                      })
                    })
                  }}
                >
                  <XIcon size={20} aria-hidden="true" />
                  <span className="sr-only">Close NOOP detail</span>
                </Button>
              </div>
              {data.detail.state === "available" ? (
                <NoopDetail data={data.detail.data} />
              ) : (
                <ReadFailure title="NOOP workout detail" data={data.detail} />
              )}
            </>
          )}
        </div>
      </section>
    </div>
  )
}

function ReadFailure({
  title,
  data,
}: {
  readonly title: string
  readonly data: Extract<TrainingRead<unknown>, { state: "unavailable" }>
}): React.JSX.Element {
  return (
    <div className="today-failure">
      <h2>{title} unavailable</h2>
      <p role="status">{trainingFailureMessage[data.reason]}</p>
    </div>
  )
}

/** Provenance for assistive technology; a stale sync draws no mark on the page. */
function Sync({
  data,
}: {
  readonly data: RecoveryDay["noop"]
}): React.JSX.Element {
  return (
    <p className="sr-only" role="status">
      Freshness: {data.freshness}
      {Object.entries(data.coverage)
        .map(([stream, state]) => ` · ${stream.replaceAll("_", " ")}: ${state}`)
        .join("")}
    </p>
  )
}

/** Charge, Rest and Effort for the selected day. */
function Rings({
  data,
  missing,
}: {
  readonly data: RecoveryDay
  readonly missing: string
}): React.JSX.Element {
  return (
    <div className="today-rings">
      <RecoveryRings data={data} missing={missing} />
    </div>
  )
}

/** The three day rings, shared by Recovery and Home; a glyph names each ring, the words stay for AT. */
export function RecoveryRings({
  data,
  missing,
}: {
  readonly data: RecoveryDay
  readonly missing: string
}): React.JSX.Element {
  const charge = data.derived_scores.recovery.value
  const rest = data.derived_scores.rest.value
  const effort = data.derived_scores.strain.value

  return (
    <>
      <ScoreRing
        kind="charge"
        icon={LightningIcon}
        share={ringShare(charge, 100)}
        band={chargeBand(charge)}
        figure={charge === null ? null : String(Math.round(charge))}
        label="Charge"
        spoken={charge === null ? missing : `${Math.round(charge)} % recovery`}
      />
      <ScoreRing
        kind="rest"
        icon={MoonIcon}
        share={ringShare(rest, 100)}
        band={chargeBand(rest)}
        figure={rest === null ? null : String(Math.round(rest))}
        label="Rest"
        spoken={rest === null ? missing : `${Math.round(rest)} % rest`}
      />
      <ScoreRing
        kind="effort"
        icon={HeartbeatIcon}
        share={ringShare(effort, 100)}
        band={effort === null ? "none" : "fair"}
        figure={effort === null ? null : String(Math.round(effort))}
        label="Effort"
        spoken={
          effort === null ? missing : `strain ${Math.round(effort)} of 100`
        }
      />
    </>
  )
}
