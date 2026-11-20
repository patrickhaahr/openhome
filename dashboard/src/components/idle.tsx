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
import { slideMemory, wantsIdleNow, type IdleSettings } from "@/lib/idle"

export const WAKE_GRACE_MS = 450

const WAKE_LIMIT_MS = 3000

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
  | { readonly tag: "waking"; readonly restoreFocus: Element | null }

type Room = {
  readonly phase: Phase
  readonly blockers: ReadonlySet<symbol>
}

type RoomEvent =
  | { readonly type: "inactive"; readonly focus: Element | null }
  | { readonly type: "input" }
  | { readonly type: "settled" }
  | { readonly type: "blocked"; readonly token: symbol }
  | { readonly type: "released"; readonly token: symbol }

const initialRoom: Room = {
  phase: { tag: "awake", handBack: null },
  blockers: new Set(),
}

function wokenTo(phase: Phase): Phase {
  return {
    tag: "awake",
    handBack: phase.tag === "awake" ? null : phase.restoreFocus,
  }
}

function reduceRoom(room: Room, event: RoomEvent): Room {
  const { phase, blockers } = room

  switch (event.type) {
    case "inactive":
      if (phase.tag !== "awake" || blockers.size > 0) return room

      return { phase: { tag: "idle", restoreFocus: event.focus }, blockers }

    case "input":
      if (phase.tag !== "idle") return room

      return {
        phase: { tag: "waking", restoreFocus: phase.restoreFocus },
        blockers,
      }

    case "settled":
      if (phase.tag === "awake") return room

      return { phase: wokenTo(phase), blockers }

    case "blocked":
      return {
        phase: phase.tag === "awake" ? phase : wokenTo(phase),
        blockers: new Set(blockers).add(event.token),
      }

    case "released": {
      if (!blockers.has(event.token)) return room
      const remaining = new Set(blockers)
      remaining.delete(event.token)

      return { phase, blockers: remaining }
    }
  }
}

export function IdleProvider({
  settings,
  overlay,
  sources = serverSources,
  children,
}: {
  settings: IdleSettings
  overlay?: React.ReactNode
  sources?: IdleSources
  children: React.ReactNode
}): React.JSX.Element {
  const [{ phase, blockers }, send] = useReducer(reduceRoom, initialRoom)
  const [memory] = useState(slideMemory)
  const screen = useRef<HTMLDivElement>(null)
  const { timeoutMs, preview } = settings
  const awake = phase.tag === "awake"
  const showing = !awake
  const blocked = blockers.size > 0

  const block = useCallback(() => {
    const token = Symbol("idle block")
    send({ type: "blocked", token })

    return () => send({ type: "released", token })
  }, [])

  useEffect(() => {
    if (!awake || blocked || timeoutMs === null) return
    let last = Date.now()
    let timer: ReturnType<typeof setTimeout> | undefined

    const touched = (): void => {
      last = Date.now()
    }

    function check(): void {
      const left = last + (timeoutMs ?? 0) - Date.now()

      if (left <= 0) send({ type: "inactive", focus: document.activeElement })
      else timer = setTimeout(check, left)
    }

    for (const type of activity)
      window.addEventListener(type, touched, { capture: true, passive: true })
    timer = setTimeout(check, timeoutMs)

    return () => {
      clearTimeout(timer)

      for (const type of activity)
        window.removeEventListener(type, touched, { capture: true })
    }
  }, [awake, blocked, timeoutMs])

  useEffect(() => {
    if (!wantsIdleNow({ timeoutMs, preview }, window.location.search)) return

    const timer = setTimeout(() => {
      send({ type: "inactive", focus: document.activeElement })
    }, 0)

    return () => clearTimeout(timer)
  }, [timeoutMs, preview])

  useEffect(() => {
    if (phase.tag === "idle") {
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

  const state = useMemo(() => ({ phase: phase.tag, block }), [phase.tag, block])

  return (
    <IdleContext value={state}>
      {children}
      {showing ? (
        <IdleScreen
          ref={screen}
          waking={phase.tag === "waking"}
          memory={memory}
          overlay={overlay}
          sources={sources}
        />
      ) : null}
    </IdleContext>
  )
}
