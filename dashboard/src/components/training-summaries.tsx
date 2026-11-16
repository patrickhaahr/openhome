import { useRef, useState } from "react"
import { HeartbeatIcon } from "@phosphor-icons/react/dist/csr/Heartbeat"
import { LightningIcon } from "@phosphor-icons/react/dist/csr/Lightning"
import { MoonIcon } from "@phosphor-icons/react/dist/csr/Moon"
import { XIcon } from "@phosphor-icons/react/dist/csr/X"
import { Button } from "@/components/ui/button"
import { NoopDetail } from "@/components/noop-detail"
import { DayStrip } from "@/components/day-strip"
import { ScoreRing, type RingSize } from "@/components/score-ring"
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
import { FailurePlate, WorkoutFeedback } from "@/components/workout-parts"

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
    <div data-slot="today" className="grid gap-today" aria-busy={busy}>
      <h2 className="sr-only">Recovery day · {data.day}</h2>
      <WorkoutFeedback failed={!busy && feedback !== "" ? true : undefined}>
        {busy ? "Reading training data…" : feedback}
      </WorkoutFeedback>
      <div className="grid min-h-fit-screen grid-cols-2 gap-workout-gutter *:min-w-0 max-[900px]:min-h-0 max-[900px]:grid-cols-1">
        <div className="relative grid grid-rows-fill-hug gap-4 max-[900px]:gap-6">
          {data.recovery.state === "available" ? (
            <>
              <Sync data={data.recovery.data.noop} />
              <Rings data={data.recovery.data} missing={missing} />
            </>
          ) : (
            <ReadFailure
              title="Recovery day"
              data={data.recovery}
              className="self-center"
            />
          )}
          <div className="min-w-0">
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
      <section className="grid gap-4" aria-label="Sessions">
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
          className="relative scroll-mt-detail-top focus:outline-none"
        >
          {data.detail.state === "none" ? null : (
            <>
              <div className="absolute top-4 right-4 z-1">
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
  className = "",
}: {
  readonly title: string
  readonly data: Extract<TrainingRead<unknown>, { state: "unavailable" }>
  readonly className?: string
}): React.JSX.Element {
  return (
    <FailurePlate title={`${title} unavailable`} className={className}>
      <p className="text-muted-ink" role="status">
        {trainingFailureMessage[data.reason]}
      </p>
    </FailurePlate>
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
    <div className="grid grid-cols-rings justify-center gap-rings self-center">
      <RecoveryRings data={data} missing={missing} size="hero" />
    </div>
  )
}

/** The three day rings, shared by Recovery and Home; a glyph names each ring, the words stay for AT. */
export function RecoveryRings({
  data,
  missing,
  size,
}: {
  readonly data: RecoveryDay
  readonly missing: string
  readonly size: RingSize
}): React.JSX.Element {
  const charge = data.derived_scores.recovery.value
  const rest = data.derived_scores.rest.value
  const effort = data.derived_scores.strain.value

  return (
    <>
      <ScoreRing
        kind="charge"
        size={size}
        icon={LightningIcon}
        share={ringShare(charge, 100)}
        band={chargeBand(charge)}
        figure={charge === null ? null : String(Math.round(charge))}
        label="Charge"
        spoken={charge === null ? missing : `${Math.round(charge)} % recovery`}
      />
      <ScoreRing
        kind="rest"
        size={size}
        stagger={size === "hero" ? 1 : 0}
        icon={MoonIcon}
        share={ringShare(rest, 100)}
        band={chargeBand(rest)}
        figure={rest === null ? null : String(Math.round(rest))}
        label="Rest"
        spoken={rest === null ? missing : `${Math.round(rest)} % rest`}
      />
      <ScoreRing
        kind="effort"
        size={size}
        stagger={size === "hero" ? 2 : 0}
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
