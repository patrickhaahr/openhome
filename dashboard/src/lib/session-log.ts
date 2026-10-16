/**
 * The Log shows one entry per training session. A logged Workout and a NOOP strap Workout that
 * are the same session become one entry; everything else stands alone.
 *
 * The pairing is presentation, not a domain claim: Axum never matches the two (CONTEXT.md), and
 * a logged Workout only has a calendar date, so there is no time to overlap. The rule is:
 *
 * 1. Only the same Copenhagen Training Day can pair; a near miss across midnight does not.
 * 2. Only strength-style strap sessions are candidates. Sports that cover ground (running,
 *    walking, cycling and the like) are movement, never the exercises that were logged.
 * 3. One-to-one. When a day has more candidates than logged Workouts the longest ones win (a
 *    warm-up walk loses to the training session); equal lengths go to the earlier start.
 * 4. The remaining candidates pair with that day's Workouts in order: the Workout logged first
 *    with the strap session that started first.
 */

import { trainingDayOn } from "./workout"

/** The part of a logged Workout the pairing reads. */
export type PairableWorkout = {
  readonly id: number
  readonly date: string
}

/** The part of a NOOP Workout summary the pairing reads. */
export type PairableStrap = {
  readonly start: string
  readonly sport: string
  readonly duration_min: number | null
}

/** One Log entry: the logged Workout, the strap session, or both. */
export type Session<W extends PairableWorkout, S extends PairableStrap> = {
  /** The Copenhagen Training Day the entry sits on. */
  readonly day: string
  readonly workout: W | undefined
  readonly strap: S | undefined
}

/** Sports that cover ground; a logged lifting or calisthenics Workout is never one of these. */
const movement =
  /\b(run|running|jog|jogging|walk|walking|hike|hiking|trek|cycle|cycling|bike|biking|ride|riding|swim|swimming|row|rowing|ski|skiing|skate|skating|commute)\b/i

/**
 * Whether a strap session can be the same session as a logged Workout.
 *
 * @param sport - The NOOP sport name.
 * @returns False for sports that cover ground.
 */
export function canPair(sport: string): boolean {
  return !movement.test(sport)
}

function dayOf(strap: PairableStrap): string {
  return trainingDayOn(new Date(strap.start))
}

function earlier(a: PairableStrap, b: PairableStrap): number {
  return Date.parse(a.start) - Date.parse(b.start)
}

/** Longest first; equal lengths keep the earlier start first. */
function byLengthThenStart(a: PairableStrap, b: PairableStrap): number {
  return (b.duration_min ?? 0) - (a.duration_min ?? 0) || earlier(a, b)
}

/**
 * Merge logged Workouts and strap sessions into Log entries, newest first.
 *
 * @param workouts - Logged Workouts in any order.
 * @param straps - NOOP Workout summaries in any order.
 * @returns One entry per session; the entries of a day run from its latest to its earliest.
 */
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

  // A strap session has a start; a Workout alone only has its day, so it sorts as the day's first.
  const clock = (session: Session<W, S>) =>
    session.strap === undefined ? 0 : Date.parse(session.strap.start)

  return sessions.sort(
    (a, b) =>
      b.day.localeCompare(a.day) ||
      clock(b) - clock(a) ||
      (b.workout?.id ?? 0) - (a.workout?.id ?? 0)
  )
}
