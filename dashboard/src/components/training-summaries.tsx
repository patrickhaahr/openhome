import { useRef, useState } from "react"
import { ArrowClockwiseIcon } from "@phosphor-icons/react/dist/csr/ArrowClockwise"
import { BarbellIcon } from "@phosphor-icons/react/dist/csr/Barbell"
import { BedIcon } from "@phosphor-icons/react/dist/csr/Bed"
import { CaretLeftIcon } from "@phosphor-icons/react/dist/csr/CaretLeft"
import { CaretRightIcon } from "@phosphor-icons/react/dist/csr/CaretRight"
import { HeartbeatIcon } from "@phosphor-icons/react/dist/csr/Heartbeat"
import { MoonIcon } from "@phosphor-icons/react/dist/csr/Moon"
import { NotebookIcon } from "@phosphor-icons/react/dist/csr/Notebook"
import { PersonSimpleRunIcon } from "@phosphor-icons/react/dist/csr/PersonSimpleRun"
import { SunIcon } from "@phosphor-icons/react/dist/csr/Sun"
import { XIcon } from "@phosphor-icons/react/dist/csr/X"
import { Button } from "@/components/ui/button"
import { NoopDetail } from "@/components/noop-detail"
import { DayRings, ScoreRing } from "@/components/score-ring"
import { Sparkline } from "@/components/sparkline"
import {
  chargeBand,
  clockDuration,
  restBand,
  restReferenceMin,
  ringShare,
  shortDuration,
  spokenDuration,
  type Band,
} from "@/lib/recovery-score"
import {
  shiftDay,
  trainingFailureMessage,
  type MetricTrend,
  type NoopSelection,
  type RecoveryDay,
  type TrainingContext,
  type TrainingRead,
  type TrainingSearch,
  type TrainingState,
} from "@/lib/training"

const decimal = new Intl.NumberFormat("da-DK", { maximumFractionDigits: 1 })

const clock = new Intl.DateTimeFormat("en-GB", {
  timeZone: "Europe/Copenhagen",
  hour: "2-digit",
  minute: "2-digit",
  hourCycle: "h23",
})

const weekday = new Intl.DateTimeFormat("en-GB", {
  timeZone: "UTC",
  weekday: "narrow",
})

const dayLabel = new Intl.DateTimeFormat("en-GB", {
  timeZone: "UTC",
  weekday: "short",
  day: "numeric",
  month: "short",
})

/** Training Days are calendar dates; formatting them at UTC midnight never shifts the day. */
function calendar(day: string, format: Intl.DateTimeFormat): string {
  return format.format(new Date(`${day}T00:00:00Z`))
}

function noopButtonId(selection: NoopSelection): string {
  return `noop-workout-${JSON.stringify([selection.source, selection.start, selection.sport])}`
}

/** One day's three readings, ready to draw as rings. */
type DayReadings = {
  readonly charge: number | null
  readonly rest: number | null
  readonly effort: number | null
}

function readings(read: TrainingRead<RecoveryDay>): DayReadings {
  if (read.state === "unavailable")
    return { charge: null, rest: null, effort: null }

  return {
    charge: read.data.derived_scores.recovery.value,
    rest: read.data.sleep.total.value,
    effort: read.data.derived_scores.strain.value,
  }
}

function spokenReadings(values: DayReadings): string {
  return [
    `Charge ${values.charge === null ? "not recorded" : `${Math.round(values.charge)} %`}`,
    `Rest ${values.rest === null ? "not recorded" : spokenDuration(values.rest)}`,
    `Effort ${values.effort === null ? "not recorded" : Math.round(values.effort)}`,
  ].join(", ")
}

/** The selected day as rings, then its night, its body and its sessions. Words stay for AT. */
export function TrainingSummaries({
  data,
  selectDay,
  open,
  openLogged,
  refresh,
}: {
  readonly data: TrainingState
  readonly selectDay: (day: string) => Promise<void>
  readonly open: (selection: TrainingSearch["noop"]) => Promise<void>
  /** Show a logged Workout in the log view. */
  readonly openLogged: (workoutId: number) => Promise<void>
  readonly refresh: () => Promise<void>
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

  return (
    <div className="today" aria-busy={busy}>
      <h2 className="sr-only">Recovery day · {data.day}</h2>
      <div className="today-week">
        <Button
          variant="ghost"
          size="key-icon"
          title="Previous week"
          disabled={busy}
          onClick={() => void run(() => selectDay(shiftDay(data.day, -7)))}
        >
          <CaretLeftIcon size={22} aria-hidden="true" />
          <span className="sr-only">Previous week</span>
        </Button>
        <ol className="today-days" aria-label="Training days">
          {data.week.map((entry) => {
            const values = readings(entry.recovery)
            const current = entry.day === data.day

            return (
              <li key={entry.day}>
                <button
                  ref={current ? dayButton : undefined}
                  type="button"
                  className="today-day"
                  aria-pressed={current}
                  data-today={entry.day === today}
                  disabled={busy}
                  aria-label={`${calendar(entry.day, dayLabel)}: ${spokenReadings(values)}`}
                  onClick={() =>
                    void run(() => (current ? refresh() : selectDay(entry.day)))
                  }
                >
                  <span className="today-day-name" aria-hidden="true">
                    {calendar(entry.day, weekday)}
                  </span>
                  <DayRings
                    charge={{
                      share: ringShare(values.charge, 100),
                      band: chargeBand(values.charge),
                    }}
                    rest={{
                      share: ringShare(values.rest, restReferenceMin),
                      band: restBand(values.rest),
                    }}
                    effort={{ share: ringShare(values.effort, 100) }}
                  />
                  <span className="today-day-date" aria-hidden="true">
                    {Number(entry.day.slice(8))}
                  </span>
                </button>
              </li>
            )
          })}
        </ol>
        <Button
          variant="ghost"
          size="key-icon"
          title="Next week"
          disabled={busy || data.day >= today}
          onClick={() =>
            void run(() => {
              const next = shiftDay(data.day, 7)

              return selectDay(next > today ? today : next)
            })
          }
        >
          <CaretRightIcon size={22} aria-hidden="true" />
          <span className="sr-only">Next week</span>
        </Button>
        <div className="today-tools">
          {data.recovery.state === "available" ? (
            <SyncMark
              freshness={data.recovery.data.noop.freshness}
              lastPush={data.recovery.data.noop.last_push_at}
              coverage={data.recovery.data.noop.coverage}
            />
          ) : null}
          <Button
            type="button"
            variant="outline"
            size="key-icon"
            title="Refresh training"
            data-spinning={busy}
            disabled={busy}
            onClick={() => void run(refresh)}
          >
            <ArrowClockwiseIcon size={20} aria-hidden="true" />
            <span className="sr-only">Refresh training</span>
          </Button>
        </div>
      </div>
      <p
        className="workout-feedback"
        role="status"
        data-failed={!busy && feedback !== "" ? true : undefined}
      >
        {busy ? "Reading training data…" : feedback}
      </p>
      {data.recovery.state === "available" ? (
        <Recovery
          data={data.recovery.data}
          week={data.week}
          trends={data.trends}
        />
      ) : (
        <ReadFailure title="Recovery day" data={data.recovery} />
      )}
      <section className="today-sessions" aria-label="Sessions">
        {data.context.state === "unavailable" ? (
          <ReadFailure title="Training context" data={data.context} />
        ) : (
          <Sessions
            day={contextDay}
            selected={selected}
            disabled={busy}
            openLogged={(workoutId) => void run(() => openLogged(workoutId))}
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

const freshnessWords: Record<RecoveryDay["noop"]["freshness"], string> = {
  confirmed: "Confirmed",
  partial: "Partly synced",
  unconfirmed: "Not yet confirmed",
  unknown: "Unknown",
}

/** Provenance as a mark: a filled ring is a confirmed sync, a hollow one is not yet. */
function SyncMark({
  freshness,
  lastPush,
  coverage,
}: {
  readonly freshness: RecoveryDay["noop"]["freshness"]
  readonly lastPush: string | null
  readonly coverage: Readonly<Record<string, string>>
}): React.JSX.Element {
  return (
    <p className="today-sync" data-freshness={freshness}>
      <span className="today-sync-mark" aria-hidden="true" />
      <span className="sr-only">Freshness: {freshness} · </span>
      <span title={freshnessWords[freshness]}>
        {lastPush === null ? "–" : clock.format(new Date(lastPush))}
      </span>
      <span className="sr-only">
        {Object.entries(coverage)
          .map(
            ([stream, state]) => ` · ${stream.replaceAll("_", " ")}: ${state}`
          )
          .join("")}
      </span>
    </p>
  )
}

function Recovery({
  data,
  week,
  trends,
}: {
  readonly data: RecoveryDay
  readonly week: TrainingState["week"]
  readonly trends: TrainingState["trends"]
}): React.JSX.Element {
  const charge = data.derived_scores.recovery.value
  const rest = data.sleep.total.value
  const effort = data.derived_scores.strain.value

  const covered =
    data.noop.coverage.daily_metrics === "covered" &&
    data.noop.coverage.sleep_sessions === "covered"

  const missing = covered ? "Not recorded" : "Not synced or unavailable"

  const weekly = (pick: (day: RecoveryDay) => number | null) =>
    week.map((entry) => ({
      day: entry.day,
      value:
        entry.recovery.state === "available" ? pick(entry.recovery.data) : null,
    }))

  return (
    <>
      <div className="today-rings">
        <ScoreRing
          kind="charge"
          share={ringShare(charge, 100)}
          band={chargeBand(charge)}
          figure={charge === null ? null : String(Math.round(charge))}
          unit="%"
          label="Charge"
          spoken={
            charge === null ? missing : `${Math.round(charge)} % recovery`
          }
        />
        <ScoreRing
          kind="rest"
          share={ringShare(rest, restReferenceMin)}
          band={restBand(rest)}
          figure={rest === null ? null : clockDuration(rest)}
          label="Rest"
          spoken={rest === null ? missing : `${spokenDuration(rest)} asleep`}
        />
        <ScoreRing
          kind="effort"
          share={ringShare(effort, 100)}
          band={effort === null ? "none" : "fair"}
          figure={effort === null ? null : String(Math.round(effort))}
          label="Effort"
          spoken={
            effort === null ? missing : `strain ${Math.round(effort)} of 100`
          }
        />
      </div>
      <Night data={data} trend={trends.sleep_duration} missing={missing} />
      <section className="today-vitals" aria-label="Body">
        <Vital
          label="HRV"
          unit="ms"
          value={data.hrv_rmssd.value}
          points={trendPoints(trends.hrv)}
          better="higher"
          missing={missing}
        />
        <Vital
          label="Resting HR"
          unit="bpm"
          value={data.resting_hr.value}
          points={trendPoints(trends.resting_heart_rate)}
          better="lower"
          missing={missing}
        />
        <Vital
          label="Breathing"
          unit="/min"
          value={data.respiratory_rate.value}
          points={weekly((day) => day.respiratory_rate.value)}
          better="neither"
          missing={missing}
        />
        <Vital
          label="Skin temp"
          unit="°C"
          value={data.skin_temp_deviation.value}
          points={weekly((day) => day.skin_temp_deviation.value)}
          better="neither"
          signed
          missing={missing}
        />
      </section>
      {data.journal.length === 0 ? null : (
        <ul className="today-journal" aria-label={`Journal · ${data.day}`}>
          {data.journal.map((entry) => (
            <li key={entry.question} data-yes={entry.answered_yes}>
              <span aria-hidden="true">{entry.answered_yes ? "✓" : "✕"}</span>
              <span>
                {entry.question}
                <span className="sr-only">
                  : {entry.answered_yes ? "Yes" : "No"}
                </span>
              </span>
              {entry.numeric_value === null ? null : (
                <strong>{decimal.format(entry.numeric_value)}</strong>
              )}
              {entry.notes === null ? null : <em>{entry.notes}</em>}
            </li>
          ))}
        </ul>
      )}
    </>
  )
}

function trendPoints(
  read: TrainingRead<MetricTrend>
): ReadonlyArray<{ readonly day: string; readonly value: number | null }> {
  return read.state === "available" ? read.data.daily_points : []
}

const stageOrder = ["deep", "rem", "light", "awake"] as const

const stageNames: Record<(typeof stageOrder)[number], string> = {
  deep: "Deep",
  rem: "REM",
  light: "Light",
  awake: "Awake",
}

/** The night: when it happened on a clock axis, what it was made of, and the month behind it. */
function Night({
  data,
  trend,
  missing,
}: {
  readonly data: RecoveryDay
  readonly trend: TrainingRead<MetricTrend>
  readonly missing: string
}): React.JSX.Element {
  const sessions = data.sleep.sessions

  const awake = sessions.reduce(
    (sum, session) => sum + (session.stage_min?.awake ?? 0),
    0
  )

  const stages = {
    deep: data.sleep.deep.value,
    rem: data.sleep.rem.value,
    light: data.sleep.light.value,
    awake: sessions.some((session) => session.stage_min !== null)
      ? awake
      : null,
  }

  const staged = stageOrder.reduce(
    (sum, stage) => sum + (stages[stage] ?? 0),
    0
  )

  const efficiency = data.sleep.efficiency.value
  const disturbances = data.sleep.disturbances.value

  return (
    <section className="today-night" aria-label="Sleep">
      <div className="today-night-head">
        <MoonIcon size={22} weight="fill" aria-hidden="true" />
        {sessions.length === 0 ? (
          <p data-gap="true">
            <span className="sr-only">
              {data.noop.coverage.sleep_sessions === "covered"
                ? "No sleep sessions recorded for this night."
                : "Sleep sessions have unknown coverage; they may not have synced."}
            </span>
          </p>
        ) : (
          <SleepClock sessions={sessions} />
        )}
        <dl className="today-night-stats">
          <div>
            <dt>Efficiency</dt>
            <dd data-gap={efficiency === null}>
              {efficiency === null
                ? missing
                : `${Math.round(efficiency * 100)}%`}
            </dd>
          </div>
          <div>
            <dt>Wake-ups</dt>
            <dd data-gap={disturbances === null}>
              {disturbances === null ? missing : disturbances}
            </dd>
          </div>
        </dl>
      </div>
      {staged === 0 ? null : (
        <>
          <div className="today-stages" aria-hidden="true">
            {stageOrder.map((stage) =>
              (stages[stage] ?? 0) > 0 ? (
                <span
                  key={stage}
                  data-stage={stage}
                  // SAFETY: CSSProperties omits custom properties; --minutes is the stage's share weight.
                  style={
                    { "--minutes": stages[stage] ?? 0 } as React.CSSProperties
                  }
                />
              ) : null
            )}
          </div>
          <dl className="today-stage-legend">
            {stageOrder.map((stage) => {
              const minutes = stages[stage]

              return (
                <div key={stage} data-stage={stage}>
                  <dt>{stageNames[stage]}</dt>
                  <dd data-gap={minutes === null}>
                    {minutes === null ? missing : shortDuration(minutes)}
                  </dd>
                </div>
              )
            })}
          </dl>
        </>
      )}
      {trend.state === "available" ? (
        <NightHistory trend={trend.data} day={data.day} />
      ) : null}
    </section>
  )
}

/** Each sleep session on a shared clock axis: bedtime, wake and naps where they really fell. */
function SleepClock({
  sessions,
}: {
  readonly sessions: RecoveryDay["sleep"]["sessions"]
}): React.JSX.Element {
  const hour = 3_600_000
  const starts = sessions.map((session) => Date.parse(session.start))
  const ends = sessions.map((session) => Date.parse(session.end))
  const from = Math.floor(Math.min(...starts) / hour) * hour - hour
  const to = Math.ceil(Math.max(...ends) / hour) * hour + hour
  const span = to - from

  const main = sessions.reduce((longest, session) =>
    session.duration_min > longest.duration_min ? session : longest
  )

  return (
    <div className="sleep-clock">
      <p className="sleep-clock-times">
        <span>
          <BedIcon size={18} aria-hidden="true" />
          <span className="sr-only">Asleep </span>
          {clock.format(new Date(main.start))}
        </span>
        <span>
          <SunIcon size={18} aria-hidden="true" />
          <span className="sr-only">Awake </span>
          {clock.format(new Date(main.end))}
        </span>
      </p>
      <div className="sleep-clock-track" aria-hidden="true">
        {sessions.map((session) => {
          const start = Date.parse(session.start)
          const end = Date.parse(session.end)

          return (
            <span
              key={`${session.source}/${session.start}`}
              data-main={session === main}
              // SAFETY: CSSProperties omits custom properties; both are 0..1 shares of the clock axis.
              style={
                {
                  "--from": (start - from) / span,
                  "--length": (end - start) / span,
                } as React.CSSProperties
              }
            />
          )
        })}
      </div>
      <p className="sleep-clock-axis" aria-hidden="true">
        <span>{clock.format(new Date(from))}</span>
        <span>{clock.format(new Date(to))}</span>
      </p>
      <ul className="sr-only">
        {sessions.map((session) => (
          <li key={`${session.source}/${session.start}`}>
            {clock.format(new Date(session.start))} to{" "}
            {clock.format(new Date(session.end))},{" "}
            {spokenDuration(session.duration_min)}
            {session.user_edited ? " · User edited" : ""}
            {session.staging_sparse === true ? " · Sparse staging" : ""}
          </li>
        ))}
      </ul>
    </div>
  )
}

/** Thirty nights as columns against the eight-hour reference; the selected night is solid. */
function NightHistory({
  trend,
  day,
}: {
  readonly trend: MetricTrend
  readonly day: string
}): React.JSX.Element {
  const top = Math.max(
    restReferenceMin * 1.25,
    ...trend.daily_points.map((point) => point.value ?? 0)
  )

  const recorded = trend.daily_points.flatMap((point) =>
    point.value === null ? [] : [point.value]
  )

  const average =
    recorded.length === 0
      ? null
      : recorded.reduce((sum, value) => sum + value, 0) / recorded.length

  return (
    <figure className="night-history">
      <div
        className="night-history-bars"
        // SAFETY: CSSProperties omits custom properties; --reference is a 0..1 height read by the reference line.
        style={{ "--reference": restReferenceMin / top } as React.CSSProperties}
        aria-hidden="true"
      >
        {trend.daily_points.map((point) => (
          <span
            key={point.day}
            data-band={restBand(point.value)}
            data-selected={point.day === day}
            title={`${point.day} · ${point.value === null ? "–" : clockDuration(point.value)}`}
            // SAFETY: CSSProperties omits custom properties; --share is a 0..1 column height.
            style={
              { "--share": (point.value ?? 0) / top } as React.CSSProperties
            }
          />
        ))}
      </div>
      <figcaption>
        <span aria-hidden="true">{trend.daily_points.length}d</span>
        {average === null ? null : (
          <span>
            <span className="sr-only">
              {trend.daily_points.length}-night average:{" "}
            </span>
            <span aria-hidden="true">⌀ </span>
            {clockDuration(average)}
          </span>
        )}
      </figcaption>
    </figure>
  )
}

/** One body signal: today's figure, how it sits against its own recent days, and their shape. */
function Vital({
  label,
  unit,
  value,
  points,
  better,
  signed = false,
  missing,
}: {
  readonly label: string
  readonly unit: string
  readonly value: number | null
  readonly points: ReadonlyArray<{
    readonly day: string
    readonly value: number | null
  }>
  readonly better: "higher" | "lower" | "neither"
  readonly signed?: boolean
  readonly missing: string
}): React.JSX.Element {
  const earlier = points
    .slice(0, -1)
    .flatMap((point) => (point.value === null ? [] : [point.value]))

  const baseline =
    earlier.length < 3
      ? null
      : earlier.reduce((sum, item) => sum + item, 0) / earlier.length

  const delta = value === null || baseline === null ? null : value - baseline
  const tone = toneOf(delta, better, baseline)

  const figure =
    value === null
      ? missing
      : `${signed && value > 0 ? "+" : ""}${decimal.format(value)}`

  return (
    <div className="vital" data-tone={tone}>
      <p className="vital-label">{label}</p>
      <p className="vital-value">
        <span data-gap={value === null}>{figure}</span>
        {value === null ? null : <small>{unit}</small>}
      </p>
      {delta === null ? null : (
        <p className="vital-delta">
          <span aria-hidden="true">{delta >= 0 ? "▲" : "▼"}</span>
          <span className="sr-only">
            {delta >= 0 ? "Above" : "Below"} its recent average by{" "}
          </span>
          {decimal.format(Math.abs(delta))}
        </p>
      )}
      <Sparkline points={points} baseline={baseline} />
    </div>
  )
}

function toneOf(
  delta: number | null,
  better: "higher" | "lower" | "neither",
  baseline: number | null
): Band {
  if (delta === null || baseline === null || better === "neither") return "none"

  // Within 3 % of the recent average reads as steady rather than a change.
  if (Math.abs(delta) <= Math.abs(baseline) * 0.03) return "fair"

  return delta > 0 === (better === "higher") ? "good" : "poor"
}

function sportIcon(sport: string): React.JSX.Element {
  const name = sport.toLowerCase()

  if (/run|walk|jog|hike/.test(name))
    return <PersonSimpleRunIcon size={26} aria-hidden="true" />

  if (/calisthenic|strength|weight|lift|gym/.test(name))
    return <BarbellIcon size={26} aria-hidden="true" />

  return <HeartbeatIcon size={26} aria-hidden="true" />
}

/** The day's sessions: strap recordings open in place; logged Workouts open in the log. */
function Sessions({
  day,
  selected,
  disabled,
  open,
  openLogged,
}: {
  readonly day: TrainingContext["days"][number] | undefined
  readonly selected: NoopSelection | undefined
  readonly disabled: boolean
  readonly open: (selection: NoopSelection) => void
  readonly openLogged: (workoutId: number) => void
}): React.JSX.Element {
  if (day === undefined) return <></>

  if (day.noop_workouts.length === 0 && day.workouts.length === 0)
    return (
      <p className="today-rest-day">
        <BedIcon size={22} aria-hidden="true" />
        <span className="sr-only">
          {day.noop.workouts.coverage.workouts === "covered"
            ? "No workouts logged and no NOOP workouts recorded on this day."
            : "No workouts logged. NOOP workouts have unknown coverage; they may not have synced."}
        </span>
      </p>
    )

  return (
    <ul className="today-session-list">
      {day.noop_workouts.map((workout) => {
        const pressed =
          selected?.source === workout.source &&
          selected.start === workout.start &&
          selected.sport === workout.sport

        const strain = workout.strain_score

        return (
          <li key={`${workout.source}/${workout.start}/${workout.sport}`}>
            <button
              id={noopButtonId(workout)}
              type="button"
              className="session-tile"
              aria-pressed={pressed}
              disabled={disabled}
              onClick={() =>
                open({
                  source: workout.source,
                  start: workout.start,
                  sport: workout.sport,
                })
              }
            >
              <span className="sr-only">Open </span>
              <span className="session-tile-icon">
                {sportIcon(workout.sport)}
              </span>
              <span className="session-tile-name">{workout.sport}</span>
              <span className="sr-only"> · </span>
              <span className="session-tile-time">
                {clock.format(new Date(workout.start))}
              </span>
              <span className="session-tile-stats">
                {workout.duration_min === null ? null : (
                  <span>
                    <span className="sr-only">Duration </span>
                    {shortDuration(workout.duration_min)}
                  </span>
                )}
                {workout.avg_hr_bpm === null ? null : (
                  <span>
                    <HeartbeatIcon size={14} weight="fill" aria-hidden="true" />
                    <span className="sr-only">Average heart rate </span>
                    {Math.round(workout.avg_hr_bpm)}
                  </span>
                )}
              </span>
              <span
                className="session-tile-effort"
                // SAFETY: CSSProperties omits custom properties; --share is a 0..1 fill read by the conic ring.
                style={
                  { "--share": ringShare(strain, 100) } as React.CSSProperties
                }
              >
                <span className="sr-only">Effort </span>
                {strain === null ? "–" : Math.round(strain)}
              </span>
            </button>
          </li>
        )
      })}
      {day.workouts.map((workout) => (
        <li key={workout.id}>
          <button
            type="button"
            className="session-tile"
            data-kind="logged"
            disabled={disabled}
            onClick={() => openLogged(workout.id)}
          >
            <span className="sr-only">Open logged </span>
            <span className="session-tile-icon">
              <NotebookIcon size={26} aria-hidden="true" />
            </span>
            <span className="session-tile-name">
              {workout.name ?? "Workout"}
            </span>
            <span className="session-tile-exercises">
              {workout.exercises.map((exercise, index) => (
                <span key={`${exercise.exercise}/${index}`}>
                  {exercise.exercise}
                  <small>
                    <span className="sr-only">, sets: </span>
                    {exercise.sets}×
                  </small>
                </span>
              ))}
            </span>
          </button>
        </li>
      ))}
    </ul>
  )
}
