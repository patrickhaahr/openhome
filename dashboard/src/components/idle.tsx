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

/** After the waking gesture's last event, how long stray events (a late click) are swallowed. */
export const WAKE_GRACE_MS = 450

/** A held finger that never reports its release still wakes after this long. */
const WAKE_LIMIT_MS = 3000

/** Events from the waking gesture that must never reach a control under the slideshow. */
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

/** Activity that restarts the inactivity countdown while awake. */
const activity = ["pointerdown", "pointermove", "keydown", "wheel"] as const

/**
 * The room's phase. Each variant carries the focus it needs: a showing phase remembers where focus
 * was so waking can return it, and the awake phase that follows holds it until the focus effect
 * hands it back once.
 */
type Phase =
  | { readonly tag: "awake"; readonly handBack: Element | null }
  | { readonly tag: "idle"; readonly restoreFocus: Element | null }
  | { readonly tag: "waking"; readonly restoreFocus: Element | null }

/**
 * Phase and blockers are separate facts: a block is a reason not to enter idle, never a phase, so
 * several can be held at once and each is released on its own.
 */
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

/** Where focus returns when a showing phase ends; null when nothing was focused. */
function wokenTo(phase: Phase): Phase {
  return {
    tag: "awake",
    handBack: phase.tag === "awake" ? null : phase.restoreFocus,
  }
}

/**
 * The single transition boundary. Every timer and listener only reports an event; whether it still
 * applies is decided here against the latest state, so a callback that outlives its cause (a
 * countdown firing just after a block, a wake landing just after a block woke the room) is a no-op.
 */
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
      // Declared busy or conversation activity takes the room back at once.
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

/**
 * Watches for inactivity and shows the Idle Slideshow over the dashboard without unmounting it,
 * so the page, its scroll and any draft are exactly as they were on waking. Anything that calls
 * `useIdleBlock(true)` holds the slideshow off; a block that appears while idle wakes it.
 *
 * Timers and window listeners are effects that only report events to one reducer; each effect's
 * cleanup (phase change, block, unmount) clears them, and the reducer ignores any that still land.
 */
export function IdleProvider({
  settings,
  overlay,
  sources = serverSources,
  children,
}: {
  settings: IdleSettings
  /** Extra content for the overlay plate, beneath the clock, date and weather. */
  overlay?: React.ReactNode
  /** Where photos and weather come from; tests pass synthetic reads. */
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

  // The countdown starts afresh whenever the dashboard wakes or its last block is released.
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

  // A synthetic preview may ask for the slideshow straight away.
  useEffect(() => {
    if (!wantsIdleNow({ timeoutMs, preview }, window.location.search)) return

    const timer = setTimeout(() => {
      send({ type: "inactive", focus: document.activeElement })
    }, 0)

    return () => clearTimeout(timer)
  }, [timeoutMs, preview])

  // Focus moves onto the slideshow while it shows and returns to where it was.
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

  // While the slideshow shows, the first touch or key only wakes, and nothing of it gets through.
  // This stays one sequential protocol: the wake event starts a single settle timer, each trailing
  // event of the same gesture restarts it, and it settles once the gesture goes quiet. It is keyed
  // on `showing`, not the exact phase, so idle -> waking keeps the timer and listeners running.
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
      // The release normally settles it; a lost release must not leave the room asleep.
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
