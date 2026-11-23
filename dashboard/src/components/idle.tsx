import {
  useCallback,
  useEffect,
  useMemo,
  useReducer,
  useRef,
  useState,
} from "react"
import {
  IdleScreen,
  serverSources,
  type IdleSources,
} from "@/components/idle-screen"
import { IdleContext } from "@/components/idle-block"
import { NightScreen } from "@/components/night-screen"
import {
  DEFAULT_IDLE_SECONDS,
  isNight,
  slideMemory,
  wantsIdleNow,
  type IdleSettings,
} from "@/lib/idle"

export const WAKE_GRACE_MS = 450

const WAKE_LIMIT_MS = 3000

export const NIGHT_CHECK_MS = 5000

const trailingPointerEvents = [
  "mousedown",
  "mouseup",
  "click",
  "dblclick",
  "auxclick",
  "contextmenu",
  "touchstart",
  "touchmove",
  "touchend",
  "touchcancel",
] as const

const trailingKeyEvents = ["keyup", "keypress"] as const

const activity = ["pointerdown", "pointermove", "keydown", "wheel"] as const

type Phase =
  | { readonly tag: "awake"; readonly handBack: Element | null }
  | { readonly tag: "idle"; readonly restoreFocus: Element | null }
  | { readonly tag: "night"; readonly restoreFocus: Element | null }
  | {
      readonly tag: "waking"
      readonly from: "idle" | "night"
      readonly restoreFocus: Element | null
    }

type Gesture =
  | { readonly tag: "none" }
  | { readonly tag: "held"; readonly pointers: ReadonlySet<number> }
  | { readonly tag: "trailing" }

type Room = {
  readonly phase: Phase
  readonly gesture: Gesture
  readonly night: boolean
  readonly blockers: ReadonlySet<symbol>
}

type RoomEvent =
  | {
      readonly type: "inactive"
      readonly focus: Element | null
      readonly night: boolean
      readonly slideshow: boolean
    }
  | { readonly type: "nightfall" }
  | { readonly type: "daybreak"; readonly slideshow: boolean }
  | { readonly type: "pressed"; readonly pointer: number }
  | { readonly type: "lifted"; readonly pointer: number }
  | { readonly type: "trailed" }
  | { readonly type: "overdue" }
  | { readonly type: "settled" }
  | { readonly type: "blocked"; readonly token: symbol }
  | { readonly type: "released"; readonly token: symbol }

const initialRoom: Room = {
  phase: { tag: "awake", handBack: null },
  gesture: { tag: "none" },
  night: false,
  blockers: new Set(),
}

function wokenTo(phase: Exclude<Phase, { tag: "awake" }>): Phase {
  return { tag: "awake", handBack: phase.restoreFocus }
}

function pressedBy(gesture: Gesture, pointer: number): Gesture {
  return {
    tag: "held",
    pointers: new Set(gesture.tag === "held" ? gesture.pointers : []).add(
      pointer
    ),
  }
}

function trailedBy(gesture: Gesture): Gesture {
  return gesture.tag === "held" ? gesture : { tag: "trailing" }
}

function liftedBy(gesture: Gesture, pointer: number): Gesture {
  if (gesture.tag !== "held") return { tag: "trailing" }
  const pointers = new Set(gesture.pointers)
  pointers.delete(pointer)

  return pointers.size > 0 ? { tag: "held", pointers } : { tag: "trailing" }
}

function withGesture(room: Room, gesture: Gesture): Room {
  const { phase } = room

  return {
    ...room,
    gesture,
    phase:
      phase.tag === "idle" || phase.tag === "night"
        ? {
            tag: "waking",
            from: phase.tag,
            restoreFocus: phase.restoreFocus,
          }
        : phase,
  }
}

function uncovered(room: Room): Room {
  return room.phase.tag === "awake"
    ? room
    : { ...room, phase: wokenTo(room.phase) }
}

function reduceRoom(room: Room, event: RoomEvent): Room {
  const { phase, gesture, night, blockers } = room

  switch (event.type) {
    case "inactive":
      if (phase.tag !== "awake" || blockers.size > 0) return room

      if (!event.night && !event.slideshow) return { ...room, night: false }

      return {
        ...room,
        night: event.night,
        gesture: { tag: "none" },
        phase: {
          tag: event.night ? "night" : "idle",
          restoreFocus: event.focus,
        },
      }

    case "nightfall":
      if (night) return room

      return {
        ...room,
        night: true,
        phase: phase.tag === "idle" ? { ...phase, tag: "night" } : phase,
      }

    case "daybreak":
      if (!night) return room

      return {
        ...room,
        night: false,
        phase:
          phase.tag !== "night"
            ? phase
            : event.slideshow
              ? { ...phase, tag: "idle" }
              : wokenTo(phase),
      }

    case "pressed":
      return withGesture(room, pressedBy(gesture, event.pointer))

    case "lifted":
      return withGesture(room, liftedBy(gesture, event.pointer))

    case "trailed":
      return withGesture(room, trailedBy(gesture))

    case "overdue":
      return uncovered(room)

    case "settled":
      return { ...uncovered(room), gesture: { tag: "none" } }

    case "blocked":
      return {
        ...uncovered(room),
        blockers: new Set(blockers).add(event.token),
      }

    case "released": {
      if (!blockers.has(event.token)) return room
      const remaining = new Set(blockers)
      remaining.delete(event.token)

      return { ...room, blockers: remaining }
    }
  }
}

const currentTime = (): Date => new Date()

type Guard = "everything" | "release" | "nothing"

export function IdleProvider({
  settings,
  overlay,
  sources = serverSources,
  now = currentTime,
  children,
}: {
  settings: IdleSettings
  overlay?: React.ReactNode
  sources?: IdleSources
  now?: () => Date
  children: React.ReactNode
}): React.JSX.Element {
  const [{ phase, gesture, night, blockers }, send] = useReducer(
    reduceRoom,
    initialRoom
  )

  const [memory] = useState(slideMemory)
  const screen = useRef<HTMLDivElement>(null)
  const lastActivity = useRef(0)
  const { timeoutMs, preview } = settings
  const slideshow = timeoutMs !== null
  const inactivityMs = timeoutMs ?? (night ? DEFAULT_IDLE_SECONDS * 1000 : null)
  const awake = phase.tag === "awake"
  const waking = phase.tag === "waking"

  const guard: Guard = !awake
    ? "everything"
    : gesture.tag === "none"
      ? "nothing"
      : "release"

  const blocked = blockers.size > 0

  const block = useCallback(() => {
    const token = Symbol("idle block")
    send({ type: "blocked", token })

    return () => send({ type: "released", token })
  }, [])

  useEffect(() => {
    function check(): void {
      send(
        isNight(now()) ? { type: "nightfall" } : { type: "daybreak", slideshow }
      )
    }

    check()
    const timer = setInterval(check, NIGHT_CHECK_MS)

    return () => clearInterval(timer)
  }, [now, slideshow])

  useEffect(() => {
    if (!awake || blocked) return

    function acted(): void {
      lastActivity.current = performance.now()
    }

    acted()

    for (const type of activity)
      window.addEventListener(type, acted, { capture: true, passive: true })

    return () => {
      for (const type of activity)
        window.removeEventListener(type, acted, { capture: true })
    }
  }, [awake, blocked])

  useEffect(() => {
    if (!awake || blocked || inactivityMs === null) return
    const limit = inactivityMs
    let timer: ReturnType<typeof setTimeout> | undefined

    function check(): void {
      const left = lastActivity.current + limit - performance.now()

      if (left > 0) {
        timer = setTimeout(check, left)

        return
      }

      send({
        type: "inactive",
        focus: document.activeElement,
        night: isNight(now()),
        slideshow,
      })
    }

    check()

    return () => clearTimeout(timer)
  }, [awake, blocked, inactivityMs, now, slideshow])

  useEffect(() => {
    if (!wantsIdleNow({ timeoutMs, preview }, window.location.search)) return

    const timer = setTimeout(() => {
      send({
        type: "inactive",
        focus: document.activeElement,
        night: isNight(now()),
        slideshow: true,
      })
    }, 0)

    return () => clearTimeout(timer)
  }, [timeoutMs, preview, now])

  useEffect(() => {
    if (phase.tag === "idle" || phase.tag === "night") {
      screen.current?.focus({ preventScroll: true })

      return
    }

    if (phase.tag !== "awake") return
    const target = phase.handBack

    if (target instanceof HTMLElement && target.isConnected)
      target.focus({ preventScroll: true })
  }, [phase])

  useEffect(() => {
    if (gesture.tag === "none") return

    if (gesture.tag === "held" && !waking) return

    const timer =
      gesture.tag === "held"
        ? setTimeout(() => send({ type: "overdue" }), WAKE_LIMIT_MS)
        : setTimeout(() => send({ type: "settled" }), WAKE_GRACE_MS)

    return () => clearTimeout(timer)
  }, [gesture, waking])

  useEffect(() => {
    if (guard === "nothing") return

    const trailing =
      guard === "everything"
        ? [...trailingPointerEvents, ...trailingKeyEvents]
        : trailingPointerEvents

    function swallow(event: Event): void {
      if (event.cancelable) event.preventDefault()
      event.stopImmediatePropagation()
    }

    function press(event: PointerEvent): void {
      if (guard === "release" && event.isPrimary) {
        send({ type: "settled" })

        return
      }

      swallow(event)
      send({ type: "pressed", pointer: event.pointerId })
    }

    function key(event: Event): void {
      if (guard === "release") {
        send({ type: "settled" })

        return
      }

      trail(event)
    }

    function lift(event: PointerEvent): void {
      swallow(event)
      send({ type: "lifted", pointer: event.pointerId })
    }

    function trail(event: Event): void {
      swallow(event)
      send({ type: "trailed" })
    }

    const options = { capture: true, passive: false }
    window.addEventListener("pointerdown", press, options)
    window.addEventListener("pointerup", lift, options)
    window.addEventListener("pointercancel", lift, options)
    window.addEventListener("keydown", key, options)

    for (const type of trailing) window.addEventListener(type, trail, options)

    return () => {
      window.removeEventListener("pointerdown", press, options)
      window.removeEventListener("pointerup", lift, options)
      window.removeEventListener("pointercancel", lift, options)
      window.removeEventListener("keydown", key, options)

      for (const type of trailing)
        window.removeEventListener(type, trail, options)
    }
  }, [guard])

  const cover = waking ? phase.from : phase.tag

  const state = useMemo(() => ({ phase: phase.tag, block }), [phase.tag, block])

  return (
    <IdleContext value={state}>
      {children}
      {cover === "idle" ? (
        <IdleScreen
          ref={screen}
          waking={waking}
          memory={memory}
          overlay={overlay}
          sources={sources}
        />
      ) : null}
      {cover === "night" ? <NightScreen ref={screen} waking={waking} /> : null}
    </IdleContext>
  )
}
