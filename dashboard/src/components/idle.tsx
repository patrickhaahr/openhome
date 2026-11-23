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

const swallowed = [
  "pointerup",
  "pointercancel",
  "mousedown",
  "mouseup",
  "click",
  "dblclick",
  "auxclick",
  "contextmenu",
  "touchstart",
  "touchend",
  "touchmove",
  "keyup",
  "keypress",
] as const

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

type Room = {
  readonly phase: Phase
  readonly night: boolean
  readonly blockers: ReadonlySet<symbol>
}

type RoomEvent =
  | { readonly type: "inactive"; readonly focus: Element | null }
  | { readonly type: "nightfall" }
  | { readonly type: "daybreak"; readonly slideshow: boolean }
  | { readonly type: "input" }
  | { readonly type: "settled" }
  | { readonly type: "blocked"; readonly token: symbol }
  | { readonly type: "released"; readonly token: symbol }

const initialRoom: Room = {
  phase: { tag: "awake", handBack: null },
  night: false,
  blockers: new Set(),
}

function wokenTo(phase: Phase): Phase {
  return {
    tag: "awake",
    handBack: phase.tag === "awake" ? null : phase.restoreFocus,
  }
}

function reduceRoom(room: Room, event: RoomEvent): Room {
  const { phase, night, blockers } = room

  switch (event.type) {
    case "inactive":
      if (phase.tag !== "awake" || blockers.size > 0) return room

      return {
        ...room,
        phase: { tag: night ? "night" : "idle", restoreFocus: event.focus },
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

    case "input":
      if (phase.tag !== "idle" && phase.tag !== "night") return room

      return {
        ...room,
        phase: {
          tag: "waking",
          from: phase.tag,
          restoreFocus: phase.restoreFocus,
        },
      }

    case "settled":
      if (phase.tag === "awake") return room

      return { ...room, phase: wokenTo(phase) }

    case "blocked":
      return {
        ...room,
        phase: phase.tag === "awake" ? phase : wokenTo(phase),
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
  const [{ phase, night, blockers }, send] = useReducer(reduceRoom, initialRoom)
  const [memory] = useState(slideMemory)
  const screen = useRef<HTMLDivElement>(null)
  const lastActivity = useRef(0)
  const { timeoutMs, preview } = settings
  const slideshow = timeoutMs !== null
  const inactivityMs = timeoutMs ?? (night ? DEFAULT_IDLE_SECONDS * 1000 : null)
  const awake = phase.tag === "awake"
  const showing = !awake
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

    function touched(): void {
      lastActivity.current = Date.now()
    }

    touched()

    for (const type of activity)
      window.addEventListener(type, touched, { capture: true, passive: true })

    return () => {
      for (const type of activity)
        window.removeEventListener(type, touched, { capture: true })
    }
  }, [awake, blocked])

  useEffect(() => {
    if (!awake || blocked || inactivityMs === null) return
    const limit = inactivityMs
    let timer: ReturnType<typeof setTimeout> | undefined

    function check(): void {
      const left = lastActivity.current + limit - Date.now()

      if (left <= 0) send({ type: "inactive", focus: document.activeElement })
      else timer = setTimeout(check, left)
    }

    check()

    return () => clearTimeout(timer)
  }, [awake, blocked, inactivityMs])

  useEffect(() => {
    if (!wantsIdleNow({ timeoutMs, preview }, window.location.search)) return

    const timer = setTimeout(() => {
      send({ type: "inactive", focus: document.activeElement })
    }, 0)

    return () => clearTimeout(timer)
  }, [timeoutMs, preview])

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
    if (!showing) return
    let finish: ReturnType<typeof setTimeout> | undefined

    function settle(delay: number): void {
      clearTimeout(finish)
      finish = setTimeout(() => send({ type: "settled" }), delay)
    }

    function swallow(event: Event): void {
      if (event.cancelable) event.preventDefault()
      event.stopImmediatePropagation()
    }

    function wake(event: Event): void {
      swallow(event)
      send({ type: "input" })
      settle(event.type === "keydown" ? WAKE_GRACE_MS : WAKE_LIMIT_MS)
    }

    function trail(event: Event): void {
      swallow(event)
      settle(WAKE_GRACE_MS)
    }

    const options = { capture: true, passive: false }
    window.addEventListener("pointerdown", wake, options)
    window.addEventListener("keydown", wake, options)

    for (const type of swallowed) window.addEventListener(type, trail, options)

    return () => {
      clearTimeout(finish)
      window.removeEventListener("pointerdown", wake, options)
      window.removeEventListener("keydown", wake, options)

      for (const type of swallowed)
        window.removeEventListener(type, trail, options)
    }
  }, [showing])

  const waking = phase.tag === "waking"
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
