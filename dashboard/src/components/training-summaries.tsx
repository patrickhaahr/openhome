import { useRef, useState } from "react"
import { BarbellIcon } from "@phosphor-icons/react/dist/csr/Barbell"
import { BedIcon } from "@phosphor-icons/react/dist/csr/Bed"
import { HourglassMediumIcon } from "@phosphor-icons/react/dist/csr/HourglassMedium"
import { FootprintsIcon } from "@phosphor-icons/react/dist/csr/Footprints"
import { HeartIcon } from "@phosphor-icons/react/dist/csr/Heart"
import { HeartbeatIcon } from "@phosphor-icons/react/dist/csr/Heartbeat"
import { LightningIcon } from "@phosphor-icons/react/dist/csr/Lightning"
import { MoonIcon } from "@phosphor-icons/react/dist/csr/Moon"
import { PersonSimpleRunIcon } from "@phosphor-icons/react/dist/csr/PersonSimpleRun"
import { SunIcon } from "@phosphor-icons/react/dist/csr/Sun"
import { ThermometerIcon } from "@phosphor-icons/react/dist/csr/Thermometer"
import { WaveSineIcon } from "@phosphor-icons/react/dist/csr/WaveSine"
import { WindIcon } from "@phosphor-icons/react/dist/csr/Wind"
import { XIcon } from "@phosphor-icons/react/dist/csr/X"
import type { Icon } from "@phosphor-icons/react/dist/lib/types"
import { Button } from "@/components/ui/button"
import { formatDecimal, formatSigned, formatWhole } from "@/lib/number-format"
import { Hypnogram } from "@/components/hypnogram"
import { NoopDetail } from "@/components/noop-detail"
import { DayStrip } from "@/components/day-strip"
import { ScoreRing } from "@/components/score-ring"
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
  trainingFailureMessage,
  type MetricTrend,
  type NoopSelection,
  type RecoveryDay,
  type TrainingContext,
  type TrainingRead,
  type TrainingSearch,
  type TrainingState,
} from "@/lib/training"

const clock = new Intl.DateTimeFormat("en-GB", {
  timeZone: "Europe/Copenhagen",
  hour: "2-digit",
  minute: "2-digit",
  hourCycle: "h23",
})

function noopButtonId(selection: NoopSelection): string {
  return `noop-workout-${JSON.stringify([selection.source, selection.start, selection.sport])}`
}

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

/** Charge, Rest and Effort for the selected day; a glyph names each ring, the words stay for AT. */
function Rings({
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
    <div className="today-rings">
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
    </div>
  )
}

/** The body signals and journal for the selected day. */
function Body({
  data,
  week,
  trends,
  missing,
}: {
  readonly data: RecoveryDay
  readonly week: TrainingState["week"]
  readonly trends: TrainingState["trends"]
  readonly missing: string
}): React.JSX.Element {
  // Days up to the selected one, oldest first: the baseline never looks ahead of the day shown.
  const weekly = (pick: (day: RecoveryDay) => number | null) =>
    week
      .filter((entry) => entry.day <= data.day)
      .map((entry) => ({
        day: entry.day,
        value:
          entry.recovery.state === "available"
            ? pick(entry.recovery.data)
            : null,
      }))

  return (
    <>
      <section className="today-vitals" aria-label="Body">
        <Vital
          icon={WaveSineIcon}
          label="HRV"
          unit="ms"
          value={data.hrv_rmssd.value}
          points={trendPoints(trends.hrv)}
          better="higher"
          missing={missing}
        />
        <Vital
          icon={HeartIcon}
          label="Resting HR"
          unit="bpm"
          value={data.resting_hr.value}
          points={trendPoints(trends.resting_heart_rate)}
          better="lower"
          missing={missing}
        />
        <Vital
          icon={WindIcon}
          label="Breathing"
          unit="/min"
          value={data.respiratory_rate.value}
          points={weekly((day) => day.respiratory_rate.value)}
          better="neither"
          missing={missing}
        />
        <Vital
          icon={ThermometerIcon}
          label="Skin temp"
          unit="°C"
          value={data.skin_temp_deviation.value}
          points={weekly((day) => day.skin_temp_deviation.value)}
          better="neither"
          signed
          missing={missing}
        />
        <Vital
          icon={FootprintsIcon}
          label="Steps"
          unit=""
          value={data.steps.value}
          points={weekly((day) => day.steps.value)}
          better="neither"
          grouped
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
                <strong>{formatDecimal(entry.numeric_value)}</strong>
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

/** The night: when it happened and what it was made of in time order, and the month behind it. */
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

  // The longest night with ordered stages is drawn as a hypnogram; without one the totals stand in.
  const staged = sessions
    .filter((session) => session.stages !== null && session.stages.length > 0)
    .reduce<(typeof sessions)[number] | undefined>(
      (longest, session) =>
        longest === undefined || session.duration_min > longest.duration_min
          ? session
          : longest,
      undefined
    )

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

  const total = stageOrder.reduce((sum, stage) => sum + (stages[stage] ?? 0), 0)

  const efficiency = data.sleep.efficiency.value
  const disturbances = data.sleep.disturbances.value

  return (
    <section className="today-night" aria-label="Sleep">
      <div className="today-night-head">
        <MoonIcon size={22} weight="fill" aria-hidden="true" />
        {staged !== undefined ? null : sessions.length === 0 ? (
          <p data-gap="true">
            <span className="sr-only">
              {data.noop.coverage.sleep_sessions === "covered"
                ? "No sleep sessions recorded for this night."
                : "Sleep sessions have unknown coverage; they may not have synced."}
            </span>
          </p>
        ) : (
          <SleepClock sessions={sessions} asleep={data.sleep.total.value} />
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
      {staged !== undefined ? (
        <>
          <Hypnogram session={staged} asleep={data.sleep.total.value} />
          <SleepSessionList sessions={sessions} />
        </>
      ) : total === 0 ? null : (
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

/** Every sleep session's times for AT, as the hypnogram draws only the longest staged one. */
function SleepSessionList({
  sessions,
}: {
  readonly sessions: RecoveryDay["sleep"]["sessions"]
}): React.JSX.Element {
  return (
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
  )
}

/** Each sleep session on a shared clock axis: bedtime, wake and naps where they really fell. */
function SleepClock({
  sessions,
  asleep,
}: {
  readonly sessions: RecoveryDay["sleep"]["sessions"]
  readonly asleep: number | null
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
        {asleep === null ? null : (
          <span>
            <HourglassMediumIcon size={18} aria-hidden="true" />
            <span className="sr-only">Slept </span>
            {clockDuration(asleep)}
          </span>
        )}
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
      <SleepSessionList sessions={sessions} />
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
      <div className="night-history-bars" aria-hidden="true">
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

/** One body signal: a glyph, today's figure, and how far it sits from its own recent days. */
function Vital({
  icon: VitalIcon,
  label,
  unit,
  value,
  points,
  better,
  signed = false,
  grouped = false,
  missing,
}: {
  readonly icon: Icon
  readonly label: string
  readonly unit: string
  readonly value: number | null
  readonly points: ReadonlyArray<{
    readonly day: string
    readonly value: number | null
  }>
  readonly better: "higher" | "lower" | "neither"
  readonly signed?: boolean
  readonly grouped?: boolean
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
  const format = grouped ? formatWhole : formatDecimal

  const figure =
    value === null ? missing : signed ? formatSigned(value) : format(value)

  return (
    <div className="vital" data-tone={tone}>
      <VitalIcon className="vital-icon" size={22} aria-hidden="true" />
      <p className="sr-only">{label}</p>
      <p className="vital-value num">
        <span data-gap={value === null}>{figure}</span>
        {value === null || unit === "" ? null : <small>{unit}</small>}
      </p>
      {delta === null ? null : (
        <p className="vital-delta num">
          <span aria-hidden="true">{delta >= 0 ? "▲" : "▼"}</span>
          <span className="sr-only">
            {delta >= 0 ? "Above" : "Below"} its recent average by{" "}
          </span>
          {signed
            ? formatSigned(Math.abs(delta)).replace("+", "")
            : format(Math.abs(delta))}
        </p>
      )}
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

const effortRing = 2 * Math.PI * 52

function sportIcon(sport: string): React.JSX.Element {
  const name = sport.toLowerCase()

  if (/run|walk|jog|hike/.test(name))
    return <PersonSimpleRunIcon size={26} aria-hidden="true" />

  if (/calisthenic|strength|weight|lift|gym/.test(name))
    return <BarbellIcon size={26} aria-hidden="true" />

  return <HeartbeatIcon size={26} aria-hidden="true" />
}

/** The day's strap recordings, opened in place; logged Workouts live in Workout › Log. */
function Sessions({
  day,
  selected,
  disabled,
  open,
}: {
  readonly day: TrainingContext["days"][number] | undefined
  readonly selected: NoopSelection | undefined
  readonly disabled: boolean
  readonly open: (selection: NoopSelection) => void
}): React.JSX.Element {
  if (day === undefined) return <></>

  if (day.noop_workouts.length === 0)
    return (
      <p className="today-rest-day">
        <BedIcon size={22} aria-hidden="true" />
        <span className="sr-only">
          {day.noop.workouts.coverage.workouts === "covered"
            ? "No NOOP workouts recorded on this day."
            : "NOOP workouts have unknown coverage; they may not have synced."}
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
                {workout.steps === null ? null : (
                  <span>
                    <FootprintsIcon
                      size={14}
                      weight="fill"
                      aria-hidden="true"
                    />
                    <span className="sr-only">Steps </span>
                    {formatWhole(workout.steps)}
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
              <span className="session-tile-effort">
                <svg viewBox="0 0 120 120" aria-hidden="true">
                  <circle
                    className="effort-ring-track"
                    cx="60"
                    cy="60"
                    r="52"
                  />
                  {strain === null || strain <= 0 ? null : (
                    <circle
                      className="effort-ring-arc"
                      cx="60"
                      cy="60"
                      r="52"
                      strokeDasharray={`${effortRing * ringShare(strain, 100)} ${effortRing}`}
                    />
                  )}
                </svg>
                <span className="sr-only">Effort </span>
                <span aria-hidden={strain === null}>
                  {strain === null ? "–" : Math.round(strain)}
                </span>
              </span>
            </button>
          </li>
        )
      })}
    </ul>
  )
}
