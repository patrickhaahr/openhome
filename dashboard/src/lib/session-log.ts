import { trainingDayOn } from "./workout"

export type PairableWorkout = {
  readonly id: number
  readonly date: string
}

export type PairableStrap = {
  readonly start: string
  readonly sport: string
  readonly duration_min: number | null
}

export type Session<W extends PairableWorkout, S extends PairableStrap> = {
  readonly day: string
  readonly workout: W | undefined
  readonly strap: S | undefined
}

const movement =
  /\b(run|running|jog|jogging|walk|walking|hike|hiking|trek|cycle|cycling|bike|biking|ride|riding|swim|swimming|row|rowing|ski|skiing|skate|skating|commute)\b/i

export function canPair(sport: string): boolean {
  return !movement.test(sport)
}

function dayOf(strap: PairableStrap): string {
  return trainingDayOn(new Date(strap.start))
}

function earlier(a: PairableStrap, b: PairableStrap): number {
  return Date.parse(a.start) - Date.parse(b.start)
}

function byLengthThenStart(a: PairableStrap, b: PairableStrap): number {
  return (b.duration_min ?? 0) - (a.duration_min ?? 0) || earlier(a, b)
}

export function pairSessions<
  W extends PairableWorkout,
  S extends PairableStrap,
>(workouts: ReadonlyArray<W>, straps: ReadonlyArray<S>): Array<Session<W, S>> {
  const paired = new Map<S, W>()
  const days = new Set(workouts.map((workout) => workout.date))

  for (const day of days) {
    const logged = workouts
      .filter((workout) => workout.date === day)
      .sort((a, b) => a.id - b.id)

    const candidates = straps
      .filter((strap) => canPair(strap.sport) && dayOf(strap) === day)
      .sort(byLengthThenStart)
      .slice(0, logged.length)
      .sort(earlier)

    for (const [position, strap] of candidates.entries()) {
      const workout = logged[position]

      if (workout !== undefined) paired.set(strap, workout)
    }
  }

  const matched = new Set(paired.values())

  const sessions: Array<Session<W, S>> = [
    ...straps.map((strap) => ({
      day: dayOf(strap),
      workout: paired.get(strap),
      strap,
    })),
    ...workouts.flatMap((workout) =>
      matched.has(workout)
        ? []
        : [{ day: workout.date, workout, strap: undefined }]
    ),
  ]

  const clock = (session: Session<W, S>) =>
    session.strap === undefined ? 0 : Date.parse(session.strap.start)

  return sessions.sort(
    (a, b) =>
      b.day.localeCompare(a.day) ||
      clock(b) - clock(a) ||
      (b.workout?.id ?? 0) - (a.workout?.id ?? 0)
  )
}
