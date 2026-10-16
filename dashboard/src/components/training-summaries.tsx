import { useRef, useState } from "react"
import { ArrowClockwiseIcon } from "@phosphor-icons/react/dist/csr/ArrowClockwise"
import { HeartbeatIcon } from "@phosphor-icons/react/dist/csr/Heartbeat"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import {
  detailUnavailableMessage,
  trainingFailureMessage,
  type Measurement,
  type NoopSelection,
  type NoopWorkoutDetail,
  type RecoveryDay,
  type TrainingContext,
  type TrainingRead,
  type TrainingSearch,
  type TrainingState,
} from "@/lib/training"

const number = new Intl.NumberFormat("en-GB", { maximumFractionDigits: 2 })

const time = new Intl.DateTimeFormat("en-GB", {
  timeZone: "Europe/Copenhagen",
  dateStyle: "medium",
  timeStyle: "short",
})

function at(value: string): string {
  return time.format(new Date(value))
}

function value(amount: number | null, unit: string): string {
  return amount === null ? "Not available" : `${number.format(amount)} ${unit}`
}

function noopButtonId(selection: NoopSelection): string {
  return `noop-workout-${JSON.stringify([selection.source, selection.start, selection.sport])}`
}

function Observation({
  label,
  data,
  covered,
}: {
  readonly label: string
  readonly data: Measurement
  readonly covered: boolean
}): React.JSX.Element {
  return (
    <div className="training-observation">
      <dt>{label}</dt>
      <dd data-gap={data.value === null}>
        {data.value === null
          ? covered
            ? "Not recorded"
            : "Not synced or unavailable"
          : value(data.value, data.unit)}
        <small className="sr-only">
          {data.value === null
            ? `Unit: ${data.unit}`
            : `Source: ${data.source ?? "Not available"}`}
        </small>
      </dd>
    </div>
  )
}

type SyncStatus = {
  readonly freshness: string
  readonly coverage: Readonly<Record<string, string>>
  readonly last_push_at: string | null
}

function Sync({ data }: { readonly data: SyncStatus }): React.JSX.Element {
  return (
    <div className="training-sync">
      <p className="training-sync-push" data-freshness={data.freshness}>
        <span className="sr-only">Freshness: </span>
        <strong>{data.freshness}</strong>
        <span className="sr-only"> · Last completed push: </span>
        <span aria-hidden="true"> · </span>
        {data.last_push_at === null ? "Not available" : at(data.last_push_at)}
      </p>
      <ul className="training-sync-streams">
        {Object.entries(data.coverage).map(([stream, state]) => (
          <li key={stream} data-coverage={state}>
            {stream.replaceAll("_", " ")}
            <span className="sr-only">: {state}</span>
          </li>
        ))}
      </ul>
      <p className="sr-only">
        Covered means a missing row is a recorded absence. Unknown coverage may
        mean the phone has not synced. Freshness describes these streams, not
        the whole phone sync.
      </p>
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
    <Card>
      <CardHeader>
        <h2>{title} unavailable</h2>
      </CardHeader>
      <CardContent>
        <p role="status">{trainingFailureMessage[data.reason]}</p>
      </CardContent>
    </Card>
  )
}

/** Recovery and training reads rendered independently, preserving every API-selected source and missing state. */
export function TrainingSummaries({
  data,
  selectDay,
  open,
  refresh,
}: {
  readonly data: TrainingState
  readonly selectDay: (day: string) => Promise<void>
  readonly open: (selection: TrainingSearch["noop"]) => Promise<void>
  readonly refresh: () => Promise<void>
}): React.JSX.Element {
  const [day, setDay] = useState(data.day)
  const [busy, setBusy] = useState(false)
  const locked = useRef(false)
  const detailRegion = useRef<HTMLDivElement>(null)
  const detailSelection = useRef<NoopSelection | undefined>(undefined)
  const dayButton = useRef<HTMLButtonElement>(null)
  const [feedback, setFeedback] = useState("")

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

  return (
    <div className="training-summaries" aria-busy={busy}>
      <form
        className="training-toolbar"
        onSubmit={(event) => {
          event.preventDefault()
          void run(() => selectDay(day))
        }}
      >
        <label className="workout-field" htmlFor="training-day">
          Training day
          <Input
            id="training-day"
            type="date"
            required
            value={day}
            onChange={(event) => setDay(event.target.value)}
            disabled={busy}
          />
        </label>
        <Button ref={dayButton} type="submit" disabled={busy}>
          Show day
        </Button>
        <Button
          type="button"
          variant="outline"
          size="key-icon"
          title="Refresh training"
          data-spinning={busy}
          disabled={busy}
          onClick={() => {
            void run(refresh)
          }}
        >
          <ArrowClockwiseIcon size={20} aria-hidden="true" />
          <span className="sr-only">Refresh training</span>
        </Button>
      </form>
      <p
        className="workout-feedback"
        role="status"
        data-failed={!busy && feedback !== "" ? true : undefined}
      >
        {busy ? "Reading training data…" : feedback}
      </p>
      <div className="training-overview">
        {data.recovery.state === "available" ? (
          <Recovery data={data.recovery.data} />
        ) : (
          <ReadFailure title="Recovery day" data={data.recovery} />
        )}
        <div
          ref={detailRegion}
          tabIndex={-1}
          role="region"
          aria-label="NOOP workout detail"
        >
          {data.detail.state === "none" ? (
            <Card>
              <CardContent>
                <div className="training-prompt">
                  <HeartbeatIcon size={56} weight="light" aria-hidden="true" />
                  <p className="sr-only">NOOP workout detail</p>
                  <h2>Explore a recorded session</h2>
                </div>
                <div className="sr-only">
                  <p>
                    Choose a NOOP workout in the seven-day context below to
                    inspect strap heart rate, zones and recovery.
                  </p>
                  <p>
                    Logged Workouts and NOOP Workouts are separate records.
                    Sharing a day does not match them.
                  </p>
                </div>
              </CardContent>
            </Card>
          ) : (
            <div>
              <Button
                variant="ghost"
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
                      ) {
                        // A deep-linked older session may be inside a collapsed day.
                        const dayDetails = button.closest("details")

                        if (dayDetails !== null) dayDetails.open = true
                        button.focus()
                      } else dayButton.current?.focus()
                    })
                  })
                }}
              >
                Close NOOP detail
              </Button>
              {data.detail.state === "available" ? (
                <NoopDetail data={data.detail.data} />
              ) : (
                <ReadFailure title="NOOP workout detail" data={data.detail} />
              )}
            </div>
          )}
        </div>
      </div>
      {data.context.state === "available" ? (
        <Context
          data={data.context.data}
          selected={
            data.detail.state === "available" ? data.detail.data : undefined
          }
          disabled={busy}
          open={(selection) => {
            detailSelection.current = selection
            void run(async () => {
              await open(selection)
              detailRegion.current?.focus({ preventScroll: true })
              detailRegion.current?.scrollIntoView({
                block: "start",
                behavior: "auto",
              })
            })
          }}
        />
      ) : (
        <ReadFailure title="Training context" data={data.context} />
      )}
    </div>
  )
}

function Recovery({ data }: { readonly data: RecoveryDay }): React.JSX.Element {
  const covered =
    data.noop.coverage.daily_metrics === "covered" &&
    data.noop.coverage.sleep_sessions === "covered"

  const observations: ReadonlyArray<readonly [string, Measurement]> = [
    ["Sleep duration", data.sleep.total],
    ["Resting heart rate", data.resting_hr],
    ["HRV · nightly RMSSD", data.hrv_rmssd],
    ["Skin temperature deviation", data.skin_temp_deviation],
    ["Respiratory rate", data.respiratory_rate],
  ]

  return (
    <Card>
      <div className="sr-only">
        <p>Recovery day · {data.day}</p>
        <h2>The night waking on this day</h2>
        <p>{data.time_zone} · Shared fitness data</p>
      </div>
      <CardContent>
        <dl className="training-metrics">
          {observations.map(([label, observation]) => (
            <Observation
              key={label}
              label={label}
              data={observation}
              covered={covered}
            />
          ))}
        </dl>
        <h3>NOOP derived scores</h3>
        <p className="sr-only">Model outputs from NOOP, not measurements.</p>
        <dl className="training-metrics">
          <Observation
            label="Recovery"
            data={data.derived_scores.recovery}
            covered={covered}
          />
          <Observation
            label="Strain"
            data={data.derived_scores.strain}
            covered={covered}
          />
        </dl>
        <Sync data={data.noop} />
        <details className="training-disclosure">
          <summary>Sleep details & journal</summary>
          <dl className="training-metrics">
            {(
              [
                ["Deep sleep", data.sleep.deep],
                ["REM sleep", data.sleep.rem],
                ["Light sleep", data.sleep.light],
                ["Sleep efficiency", data.sleep.efficiency],
                ["Disturbances", data.sleep.disturbances],
              ] satisfies Array<[string, Measurement]>
            ).map(([label, observation]) => (
              <Observation
                key={label}
                label={label}
                data={observation}
                covered={covered}
              />
            ))}
          </dl>
          <h3>Sleep sessions</h3>
          {data.sleep.sessions.length === 0 ? (
            <p>
              {data.noop.coverage.sleep_sessions === "covered"
                ? "No sleep sessions recorded for this night."
                : "Sleep sessions have unknown coverage; they may not have synced."}
            </p>
          ) : (
            <ul className="training-records">
              {data.sleep.sessions.map((session) => (
                <li key={`${session.source}/${session.start}`}>
                  <p>
                    {at(session.start)} → {at(session.end)}
                  </p>
                  <p>
                    {value(session.duration_min, "min")} · Source:{" "}
                    {session.source}
                    {session.user_edited ? " · User edited" : ""}
                    {session.staging_sparse === true ? " · Sparse staging" : ""}
                  </p>
                  <p>
                    Efficiency: {value(session.efficiency_fraction, "fraction")}{" "}
                    · Resting HR: {value(session.resting_hr_bpm, "beats/min")} ·
                    HRV: {value(session.hrv_rmssd_ms, "ms")}
                  </p>
                  {session.stage_min === null ? (
                    <p>No staging available.</p>
                  ) : (
                    <p>
                      {Object.entries(session.stage_min)
                        .map(
                          ([stage, minutes]) =>
                            `${stage}: ${value(minutes, "min")}`
                        )
                        .join(" · ")}
                    </p>
                  )}
                </li>
              ))}
            </ul>
          )}
          <h3>Journal · {data.day}</h3>
          <p className="sr-only">Entries keep the day logged in NOOP.</p>
          {data.journal.length === 0 ? (
            <p>
              {data.noop.coverage.journal === "covered"
                ? "No journal entries recorded for this day."
                : "Journal has unknown coverage; entries may not have synced."}
            </p>
          ) : (
            <ul className="training-records">
              {data.journal.map((entry) => (
                <li key={entry.question}>
                  <p>
                    {entry.question}: {entry.answered_yes ? "Yes" : "No"}
                    {entry.numeric_value === null
                      ? ""
                      : ` · ${number.format(entry.numeric_value)}`}
                  </p>
                  {entry.notes === null ? null : <p>{entry.notes}</p>}
                  <small>Source: {entry.source}</small>
                </li>
              ))}
            </ul>
          )}
        </details>
      </CardContent>
    </Card>
  )
}

function Context({
  data,
  selected,
  open,
  disabled,
}: {
  readonly data: TrainingContext
  readonly selected: NoopSelection | undefined
  readonly open: (selection: NoopSelection) => void
  readonly disabled: boolean
}): React.JSX.Element {
  return (
    <Card>
      <div className="sr-only">
        <p>
          Training context · {data.from_day} — {data.to_day}
        </p>
        <h2>Seven days, every day included</h2>
        <p>
          {data.time_zone} · Logged and NOOP Workouts stay separate. Select a
          day to expand its summaries.
        </p>
      </div>
      <CardContent>
        <p className="sr-only">
          NOOP sources: {data.noop.imported_device_id} /{" "}
          {data.noop.computed_device_id}
        </p>
        <div>
          {[...data.days].reverse().map((day) => (
            <details
              className="training-disclosure"
              key={day.day}
              open={day.day === data.to_day ? true : undefined}
            >
              <summary>
                <span>{day.day}</span>
                <span>
                  {day.workouts.length} logged · {day.noop_workouts.length} NOOP
                </span>
              </summary>
              <dl className="training-metrics">
                {(
                  [
                    ["Body weight", day.body_weight],
                    ["Sleep duration", day.sleep_duration],
                    ["Resting heart rate", day.resting_hr],
                    ["HRV · nightly RMSSD", day.hrv_rmssd],
                  ] satisfies Array<[string, Measurement]>
                ).map(([label, observation]) => (
                  <Observation
                    key={label}
                    label={label}
                    data={observation}
                    covered={
                      label === "Body weight" ||
                      (day.noop.recovery.coverage.daily_metrics === "covered" &&
                        day.noop.recovery.coverage.sleep_sessions === "covered")
                    }
                  />
                ))}
              </dl>
              <Sync
                data={{
                  ...day.noop.recovery,
                  last_push_at: data.noop.last_push_at.recovery,
                }}
              />
              <div className="training-day-records">
                <section aria-label={`Logged Workouts ${day.day}`}>
                  <h3>Logged Workouts</h3>
                  {day.workouts.length === 0 ? (
                    <p className="workout-muted">
                      No workouts logged on this day.
                    </p>
                  ) : (
                    <ul className="training-records">
                      {day.workouts.map((workout) => (
                        <li key={workout.id}>
                          <h4>{workout.name ?? "Workout"}</h4>
                          {workout.exercises.length === 0 ? (
                            <p>No exercises logged.</p>
                          ) : (
                            <ul>
                              {workout.exercises.map((exercise, index) => (
                                <li key={`${exercise.exercise}/${index}`}>
                                  <p>
                                    {exercise.exercise} · {exercise.sets} sets
                                  </p>
                                  <p>
                                    Best reps:{" "}
                                    {value(exercise.best_reps, "reps")} · Added
                                    weight:{" "}
                                    {exercise.best_added_weight_kg === null
                                      ? "None recorded"
                                      : value(
                                          exercise.best_added_weight_kg,
                                          "kg"
                                        )}{" "}
                                    · Hold:{" "}
                                    {value(exercise.best_hold_duration_s, "s")}{" "}
                                    · RPE: {value(exercise.best_rpe, "RPE")}
                                  </p>
                                </li>
                              ))}
                            </ul>
                          )}
                        </li>
                      ))}
                    </ul>
                  )}
                </section>
                <section aria-label={`NOOP Workouts ${day.day}`}>
                  <h3>NOOP Workouts</h3>
                  {day.noop_workouts.length === 0 ? (
                    <p className="workout-muted">
                      {day.noop.workouts.coverage.workouts === "covered"
                        ? "No NOOP workouts recorded on this day."
                        : "NOOP workouts have unknown coverage; they may not have synced."}
                    </p>
                  ) : (
                    <ul className="training-records">
                      {day.noop_workouts.map((workout) => (
                        <li
                          key={`${workout.source}/${workout.start}/${workout.sport}`}
                        >
                          <Button
                            id={noopButtonId(workout)}
                            variant="outline"
                            disabled={disabled}
                            aria-pressed={
                              selected?.source === workout.source &&
                              selected?.start === workout.start &&
                              selected?.sport === workout.sport
                            }
                            onClick={() =>
                              open({
                                source: workout.source,
                                start: workout.start,
                                sport: workout.sport,
                              })
                            }
                          >
                            Open {workout.sport} · {at(workout.start)}
                          </Button>
                          <p>
                            {at(workout.start)} → {at(workout.end)}
                          </p>
                          <p>
                            {value(workout.duration_min, "min")} · Avg HR:{" "}
                            {value(workout.avg_hr_bpm, "beats/min")} · Strain:{" "}
                            {value(workout.strain_score, "score_0_100")}
                          </p>
                          <small>
                            Source: {workout.source} · Origin:{" "}
                            {workout.origin.replaceAll("_", " ")}
                          </small>
                        </li>
                      ))}
                    </ul>
                  )}
                  <Sync
                    data={{
                      ...day.noop.workouts,
                      last_push_at: data.noop.last_push_at.workouts,
                    }}
                  />
                </section>
              </div>
            </details>
          ))}
        </div>
      </CardContent>
    </Card>
  )
}

function NoopDetail({
  data,
}: {
  readonly data: NoopWorkoutDetail
}): React.JSX.Element {
  const series = data.hr_series
  const zones = data.hr_zones
  const recovery = data.hr_recovery

  const maximumBpm =
    Math.ceil(
      Math.max(200, ...(series?.points.map((point) => point.bpm) ?? [])) / 50
    ) * 50

  const durationSeconds =
    (new Date(data.end).getTime() - new Date(data.start).getTime()) / 1000

  const unavailable = (reason: keyof NoopWorkoutDetail["unavailable"]) => {
    const code = data.unavailable[reason]

    return code === undefined
      ? "Not available."
      : detailUnavailableMessage[code]
  }

  return (
    <Card>
      <CardHeader>
        <p className="sr-only">NOOP workout detail</p>
        <h2>{data.sport}</h2>
        <p>
          {at(data.start)} → {at(data.end)}
        </p>
        <p className="sr-only">
          Source: {data.source} · Origin: {data.origin.replaceAll("_", " ")}
        </p>
      </CardHeader>
      <CardContent>
        <dl className="training-metrics">
          {(
            [
              ["Duration", data.duration_min, "min"],
              ["Distance", data.distance_m, "m"],
              ["Energy", data.energy_kcal, "kcal"],
              ["Average pace", data.avg_pace_s_per_km, "s/km"],
              ["NOOP strain · model output", data.strain_score, "score_0_100"],
              ["Average HR", data.hr.avg_bpm, "beats/min"],
              ["Minimum HR", data.hr.min_bpm, "beats/min"],
              ["Maximum HR", data.hr.max_bpm, "beats/min"],
            ] satisfies Array<[string, number | null, string]>
          ).map(([label, amount, unit]) => (
            <div className="training-observation" key={label}>
              <dt>{label}</dt>
              <dd>{value(amount, unit)}</dd>
            </div>
          ))}
        </dl>
        <p className="training-source">
          Heart rate basis:{" "}
          {data.hr.basis === "strap_samples"
            ? `strap samples (${data.noop.imported_device_id})`
            : data.hr.basis === "workout_row"
              ? `stored workout row (${data.source}); unreconciled with the strap`
              : "Not available"}
        </p>
        <Sync data={data.noop} />
        <h3>Strap heart rate</h3>
        {series === null ? (
          <p>{unavailable("hr_series")}</p>
        ) : (
          <figure className="training-heart-rate">
            <svg
              viewBox="0 0 600 200"
              role="img"
              aria-label="Strap heart rate samples in beats per minute; missing buckets remain gaps"
            >
              {Array.from(
                { length: 5 },
                (_, index) => (maximumBpm * index) / 4
              ).map((bpm) => (
                <g key={bpm}>
                  <line
                    x1="40"
                    x2="590"
                    y1={170 - (bpm / maximumBpm) * 150}
                    y2={170 - (bpm / maximumBpm) * 150}
                    className="training-chart-grid"
                  />
                  <text x="0" y={174 - (bpm / maximumBpm) * 150}>
                    {bpm}
                  </text>
                </g>
              ))}
              {series.points.map((point) => (
                <circle
                  key={point.t_s}
                  cx={40 + (point.t_s / durationSeconds) * 550}
                  cy={170 - (point.bpm / maximumBpm) * 150}
                  r="2.5"
                  className="training-chart-point"
                />
              ))}
              <text x="40" y="195">
                0 min
              </text>
              <text x="590" y="195" textAnchor="end">
                {value(durationSeconds / 60, "min")}
              </text>
            </svg>
            <figcaption>
              {series.bucket_s}-second observed buckets · beats/min ·{" "}
              {value(durationSeconds / 60, "min")} span. Missing buckets remain
              gaps.
            </figcaption>
            <details className="training-disclosure">
              <summary>Inspect observed heart rate values</summary>
              <div className="training-series-values">
                {series.points.map((point) => (
                  <p key={point.t_s}>
                    +{point.t_s} s: {value(point.bpm, "beats/min")}
                  </p>
                ))}
              </div>
            </details>
          </figure>
        )}
        <h3>Heart rate zones</h3>
        {zones === null ? (
          <p>{unavailable("hr_zones")}</p>
        ) : (
          <div className="training-zones">
            <p>
              Profile maximum: {value(zones.hrmax_used, "beats/min")} · Below
              zone 1: {value(zones.below_zone_min, "min")}
            </p>
            {zones.zones.map((zone) => (
              <div key={zone.zone}>
                <p>
                  Zone {zone.zone} · {zone.lower_bpm}–{zone.upper_bpm}
                  {zone.zone === 5 ? "+" : ""} beats/min
                </p>
                <meter
                  aria-label={`Zone ${zone.zone} share`}
                  min={0}
                  max={100}
                  value={zone.percent}
                />
                <p>
                  {value(zone.minutes, "min")} · {value(zone.percent, "%")}
                </p>
              </div>
            ))}
          </div>
        )}
        <h3>Heart rate recovery</h3>
        {recovery === null ? (
          <p>{unavailable("hr_recovery")}</p>
        ) : (
          <div>
            <p>End heart rate: {value(recovery.end_hr_bpm, "beats/min")}</p>
            <ul className="training-records">
              {(
                [
                  ["1 min", recovery.at_1_min],
                  ["2 min", recovery.at_2_min],
                  ["5 min", recovery.at_5_min],
                ] satisfies Array<[string, typeof recovery.at_1_min]>
              ).map(([mark, reading]) => (
                <li key={mark}>
                  {mark}:{" "}
                  {reading === null
                    ? "Too few strap samples; not interpolated."
                    : `${value(reading.hr_bpm, "beats/min")} · Drop: ${value(reading.drop_bpm, "beats/min")}`}
                </li>
              ))}
            </ul>
          </div>
        )}
      </CardContent>
    </Card>
  )
}
