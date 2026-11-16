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
      <p className="flex justify-center p-4 text-muted-ink">
        <BedIcon size={22} aria-hidden="true" />
        <span className="sr-only">
          {day.noop.workouts.coverage.workouts === "covered"
            ? "No NOOP workouts recorded on this day."
            : "NOOP workouts have unknown coverage; they may not have synced."}
        </span>
      </p>
    )

  return (
    <ul className="grid grid-cols-tiles gap-3">
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
              className="group/tile grid min-h-22 w-full touch-manipulation grid-cols-flanked items-center gap-x-4 rounded-sheet bg-tile px-4.5 py-4 text-left text-ink no-underline transition-tile duration-fast ease-app hover:bg-wash-strong active:scale-98 aria-pressed:bg-ink aria-pressed:text-ground"
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
              <span className="col-start-1 row-span-2 row-start-1 grid size-12 place-items-center rounded-full bg-wash-strong group-aria-pressed/tile:bg-ground/16">
                {sportIcon(workout.sport)}
              </span>
              <span className="col-start-2 row-start-1 truncate text-large font-semibold">
                {workout.sport}
              </span>
              <span className="sr-only"> · </span>
              <span className="col-start-2 row-start-1 mr-1 justify-self-end text-small tabular-nums opacity-60">
                {clock.format(new Date(workout.start))}
              </span>
              <span className="col-start-2 row-start-2 flex gap-4 text-control tabular-nums opacity-70">
                {workout.duration_min === null ? null : (
                  <span className="inline-flex items-center gap-1">
                    <span className="sr-only">Duration </span>
                    {shortDuration(workout.duration_min)}
                  </span>
                )}
                {workout.steps === null ? null : (
                  <span className="inline-flex items-center gap-1">
                    <FootprintsIcon
                      className="text-poor"
                      size={14}
                      weight="fill"
                      aria-hidden="true"
                    />
                    <span className="sr-only">Steps </span>
                    {formatWhole(workout.steps)}
                  </span>
                )}
                {workout.avg_hr_bpm === null ? null : (
                  <span className="inline-flex items-center gap-1">
                    <HeartbeatIcon
                      className="text-poor"
                      size={14}
                      weight="fill"
                      aria-hidden="true"
                    />
                    <span className="sr-only">Average heart rate </span>
                    {Math.round(workout.avg_hr_bpm)}
                  </span>
                )}
              </span>
              <span className="relative col-start-3 row-span-2 row-start-1 grid size-13 place-items-center text-body font-bold tabular-nums">
                <svg
                  className="absolute inset-0 size-full -rotate-90 overflow-visible"
                  viewBox="0 0 120 120"
                  aria-hidden="true"
                >
                  <circle
                    className="stroke-wash-strong"
                    cx="60"
                    cy="60"
                    r="52"
                    fill="none"
                    strokeWidth={9}
                  />
                  {strain === null || strain <= 0 ? null : (
                    <circle
                      className="stroke-effort"
                      cx="60"
                      cy="60"
                      r="52"
                      fill="none"
                      strokeWidth={9}
                      strokeLinecap="round"
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
