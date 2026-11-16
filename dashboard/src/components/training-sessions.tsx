import { clock, noopButtonId } from "@/components/training-shared"
import { BarbellIcon } from "@phosphor-icons/react/dist/csr/Barbell"
import { BedIcon } from "@phosphor-icons/react/dist/csr/Bed"
import { FootprintsIcon } from "@phosphor-icons/react/dist/csr/Footprints"
import { HeartbeatIcon } from "@phosphor-icons/react/dist/csr/Heartbeat"
import { PersonSimpleRunIcon } from "@phosphor-icons/react/dist/csr/PersonSimpleRun"
import { formatWhole } from "@/lib/number-format"
import { ringShare, shortDuration } from "@/lib/recovery-score"
import { type NoopSelection, type TrainingContext } from "@/lib/training"

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
export function Sessions({
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
