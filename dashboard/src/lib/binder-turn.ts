import { paginate } from "./binder"

export type TurnDirection = "forward" | "back"

export type TurnOutcome = "complete" | "cancel"

export const turnStep = { forward: 1, back: -1 } as const satisfies Record<
  TurnDirection,
  number
>

type Sample = { readonly x: number; readonly time: number }

export type Grip = {
  readonly edge: TurnDirection | undefined
  readonly reach: Readonly<Record<TurnDirection, number | undefined>>
}

export type Turn =
  | { readonly phase: "idle" }
  | {
      readonly phase: "pressing"
      readonly pointer: number
      readonly x: number
      readonly y: number
      readonly time: number
      readonly pocket: boolean
      readonly grip: Grip
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
      readonly grip: Grip
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

export const tapSlop = 10

export const flickSpeed = 0.5

export const restAfter = 100

export const settleDuration = 300

export function outcomeOf(progress: number, velocity: number): TurnOutcome {
  if (velocity >= flickSpeed) return "complete"

  if (velocity <= -flickSpeed) return "cancel"

  return progress >= 0.5 ? "complete" : "cancel"
}

export function grip(
  box: { readonly left: number; readonly right: number },
  x: number,
  can: Readonly<Record<TurnDirection, boolean>>
): Grip {
  const width = box.right - box.left
  const edge = width / 6

  return {
    edge:
      can.forward && x >= box.right - edge
        ? "forward"
        : can.back && x <= box.left + edge
          ? "back"
          : undefined,
    reach: {
      forward: can.forward ? Math.max(x - box.left, width / 2) : undefined,
      back: can.back ? Math.max(box.right - x, width / 2) : undefined,
    },
  }
}

export const swipeDistance = 48

export function swipeStep(dx: number, dy: number): -1 | 0 | 1 {
  if (Math.abs(dx) < swipeDistance || Math.abs(dx) <= Math.abs(dy)) return 0

  return dx < 0 ? 1 : -1
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
        return turn.grip.edge !== undefined && !turn.pocket
          ? {
              phase: "settling",
              direction: turn.grip.edge,
              outcome: "complete",
            }
          : idle

      const dx = event.x - turn.x
      const dy = event.y - turn.y

      if (Math.hypot(dx, dy) < tapSlop) return turn

      const direction = dx < 0 ? "forward" : "back"
      const reach = turn.grip.reach[direction]

      if (Math.abs(dx) <= Math.abs(dy) || reach === undefined)
        return { phase: "refused", pointer: turn.pointer }

      return {
        phase: "dragging",
        pointer: turn.pointer,
        direction,
        origin: turn.x,
        reach,
        progress: progressOf(direction, turn.x, reach, event.x),
        last: { x: event.x, time: event.time },
        velocity:
          event.time > turn.time
            ? travel(direction, turn.x, event.x) / (event.time - turn.time)
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

export type Sheet<T> = { readonly number: number; readonly entries: T[] }

export type Opening<T> = {
  readonly left: Sheet<T>
  readonly right: Sheet<T>
  readonly lifted?: {
    readonly side: "left" | "right"
    readonly back: Sheet<T>
    readonly under: Sheet<T>
  }
}

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
