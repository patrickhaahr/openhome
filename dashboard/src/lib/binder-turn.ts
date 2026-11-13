import { paginate } from "./binder"

/** Forward lifts the right page over the spine to the next spread; back lifts the left page to the previous one. */
export type TurnDirection = "forward" | "back"

export type TurnOutcome = "complete" | "cancel"

/** The spread a turn lands on, relative to the one it starts from. */
export const turnStep = { forward: 1, back: -1 } as const satisfies Record<
  TurnDirection,
  number
>

type Sample = { readonly x: number; readonly time: number }

/** Which turn a press can start, and how far the pointer travels to lay the page on the far side. */
export type Grip = { readonly direction: TurnDirection; readonly reach: number }

/**
 * One gesture on the spread. A press may still be a tap; moving past the slop makes it a drag that
 * follows the pointer when the press gripped a page, or a refused drag that turns nothing when it
 * did not. Releasing settles a drag, and a tap in a grip away from a pocket turns the page. A settled
 * completion leaves the page turned until the next spread replaces this one; every other phase ends
 * back at idle.
 */
export type Turn =
  | { readonly phase: "idle" }
  | {
      readonly phase: "pressing"
      readonly pointer: number
      readonly x: number
      readonly y: number
      readonly time: number
      readonly pocket: boolean
      readonly grip: Grip | undefined
    }
  | {
      readonly phase: "dragging"
      readonly pointer: number
      readonly direction: TurnDirection
      readonly origin: number
      readonly reach: number
      readonly progress: number
      readonly last: Sample
      readonly velocity: number
    }
  | { readonly phase: "refused"; readonly pointer: number }
  | {
      readonly phase: "settling"
      readonly direction: TurnDirection
      readonly outcome: TurnOutcome
    }
  | { readonly phase: "turned"; readonly direction: TurnDirection }

export type TurnEvent =
  | {
      readonly type: "press"
      readonly pointer: number
      readonly x: number
      readonly y: number
      readonly time: number
      readonly pocket: boolean
      readonly grip: Grip | undefined
    }
  | {
      readonly type: "move"
      readonly pointer: number
      readonly x: number
      readonly y: number
      readonly time: number
    }
  | {
      readonly type: "release"
      readonly pointer: number
      readonly x: number
      readonly time: number
    }
  | { readonly type: "lose"; readonly pointer: number }
  | { readonly type: "flip"; readonly direction: TurnDirection }
  | { readonly type: "settled" }

export const idle: Turn = { phase: "idle" }

/** How far a press may wander, in CSS pixels, and still be a tap. */
export const tapSlop = 10

/** A release this fast toward completion, in CSS pixels per millisecond, finishes the turn wherever it is. */
export const flickSpeed = 0.5

/** A pointer held still this long, in milliseconds, before release has no speed left. */
export const restAfter = 100

/** How long a released page takes to land or spring back, in milliseconds. */
export const settleDuration = 300

/** Past the midpoint the page falls forward; a flick either way overrides it. */
export function outcomeOf(progress: number, velocity: number): TurnOutcome {
  if (velocity >= flickSpeed) return "complete"

  if (velocity <= -flickSpeed) return "cancel"

  return progress >= 0.5 ? "complete" : "cancel"
}

/** Which turn a press at `x` can start: one pocket column from either edge of the spread, never past the first or last spread. */
export function grip(
  box: { readonly left: number; readonly right: number },
  x: number,
  can: Readonly<Record<TurnDirection, boolean>>
): Grip | undefined {
  const edge = (box.right - box.left) / 6

  if (can.forward && x >= box.right - edge)
    return { direction: "forward", reach: x - box.left }

  if (can.back && x <= box.left + edge)
    return { direction: "back", reach: box.right - x }

  return undefined
}

function travel(direction: TurnDirection, origin: number, x: number): number {
  return direction === "forward" ? origin - x : x - origin
}

function progressOf(
  direction: TurnDirection,
  origin: number,
  reach: number,
  x: number
): number {
  return Math.min(1, Math.max(0, travel(direction, origin, x) / reach))
}

export function nextTurn(turn: Turn, event: TurnEvent): Turn {
  switch (turn.phase) {
    case "idle":
      if (event.type === "press")
        return {
          phase: "pressing",
          pointer: event.pointer,
          x: event.x,
          y: event.y,
          time: event.time,
          pocket: event.pocket,
          grip: event.grip,
        }

      if (event.type === "flip")
        return {
          phase: "settling",
          direction: event.direction,
          outcome: "complete",
        }

      return turn
    case "pressing":
      if (event.type === "press" || event.type === "flip") return turn

      if (event.type === "settled" || event.pointer !== turn.pointer)
        return turn

      if (event.type === "lose") return idle

      if (event.type === "release")
        return turn.grip !== undefined && !turn.pocket
          ? {
              phase: "settling",
              direction: turn.grip.direction,
              outcome: "complete",
            }
          : idle

      if (Math.hypot(event.x - turn.x, event.y - turn.y) < tapSlop) return turn

      if (turn.grip === undefined)
        return { phase: "refused", pointer: turn.pointer }

      return {
        phase: "dragging",
        pointer: turn.pointer,
        direction: turn.grip.direction,
        origin: turn.x,
        reach: turn.grip.reach,
        progress: progressOf(
          turn.grip.direction,
          turn.x,
          turn.grip.reach,
          event.x
        ),
        last: { x: event.x, time: event.time },
        velocity:
          event.time > turn.time
            ? travel(turn.grip.direction, turn.x, event.x) /
              (event.time - turn.time)
            : 0,
      }
    case "dragging": {
      if (
        event.type === "press" ||
        event.type === "flip" ||
        event.type === "settled" ||
        event.pointer !== turn.pointer
      )
        return turn

      if (event.type === "lose")
        return {
          phase: "settling",
          direction: turn.direction,
          outcome: "cancel",
        }

      const progress = progressOf(
        turn.direction,
        turn.origin,
        turn.reach,
        event.x
      )

      const elapsed = event.time - turn.last.time

      if (event.type === "release")
        return {
          phase: "settling",
          direction: turn.direction,
          outcome: outcomeOf(progress, elapsed > restAfter ? 0 : turn.velocity),
        }

      if (elapsed <= 0) return { ...turn, progress }

      return {
        ...turn,
        progress,
        last: { x: event.x, time: event.time },
        velocity: travel(turn.direction, turn.last.x, event.x) / elapsed,
      }
    }

    case "refused":
      if (
        (event.type === "release" || event.type === "lose") &&
        event.pointer === turn.pointer
      )
        return idle

      return turn

    case "settling":
      if (event.type !== "settled") return turn

      return turn.outcome === "complete"
        ? { phase: "turned", direction: turn.direction }
        : idle
    case "turned":
      return turn
  }
}

/** One page as numbered in the whole Binder, from 1. */
export type Sheet<T> = { readonly number: number; readonly entries: T[] }

/** The two pages lying open, and while one turns, the page on its back and the page it uncovers. */
export type Opening<T> = {
  readonly left: Sheet<T>
  readonly right: Sheet<T>
  readonly lifted?: {
    readonly side: "left" | "right"
    readonly back: Sheet<T>
    readonly under: Sheet<T>
  }
}

/**
 * What lies open on spread `index`. Turning forward lifts the right page, which lands as the next
 * spread's left page and uncovers its right; turning back mirrors that.
 */
export function opening<T>(
  entries: readonly T[],
  index: number,
  direction: TurnDirection | undefined
): Opening<T> {
  const here = paginate(entries, index)
  const first = here.index * 2 + 1
  const left = { number: first, entries: here.left }
  const right = { number: first + 1, entries: here.right }

  if (direction === undefined) return { left, right }

  const there = paginate(entries, here.index + turnStep[direction])
  const thereLeft = { number: there.index * 2 + 1, entries: there.left }
  const thereRight = { number: there.index * 2 + 2, entries: there.right }

  return {
    left,
    right,
    lifted:
      direction === "forward"
        ? { side: "right", back: thereLeft, under: thereRight }
        : { side: "left", back: thereRight, under: thereLeft },
  }
}
