import { clock } from "@/components/training-shared"
import { BedIcon } from "@phosphor-icons/react/dist/csr/Bed"
import { HourglassMediumIcon } from "@phosphor-icons/react/dist/csr/HourglassMedium"
import { MoonIcon } from "@phosphor-icons/react/dist/csr/Moon"
import { SunIcon } from "@phosphor-icons/react/dist/csr/Sun"
import { Hypnogram } from "@/components/hypnogram"
import {
  clockDuration,
  restBand,
  restReferenceMin,
  shortDuration,
  spokenDuration,
} from "@/lib/recovery-score"
import {
  type MetricTrend,
  type RecoveryDay,
  type TrainingRead,
} from "@/lib/training"

const stageOrder = ["deep", "rem", "light", "awake"] as const

const stageNames: Record<(typeof stageOrder)[number], string> = {
  deep: "Deep",
  rem: "REM",
  light: "Light",
  awake: "Awake",
}

/** The night: when it happened and what it was made of in time order, and the month behind it. */
export function Night({
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
          <p data-gap="true" className="gap-dash-5.5">
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
            <dd
              data-gap={efficiency === null}
              className="data-[gap=true]:gap-dash-5.5"
            >
              {efficiency === null
                ? missing
                : `${Math.round(efficiency * 100)}%`}
            </dd>
          </div>
          <div>
            <dt>Wake-ups</dt>
            <dd
              data-gap={disturbances === null}
              className="data-[gap=true]:gap-dash-5.5"
            >
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
                  <dd
                    data-gap={minutes === null}
                    className="data-[gap=true]:gap-dash-5.5"
                  >
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
