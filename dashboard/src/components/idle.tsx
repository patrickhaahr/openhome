import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import {
  IdleScreen,
  serverSources,
  type IdleSources,
} from "@/components/idle-screen"
import { IdleContext, type IdlePhase } from "@/components/idle-block"
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
 * Watches for inactivity and shows the Idle Slideshow over the dashboard without unmounting it,
 * so the page, its scroll and any draft are exactly as they were on waking. Anything that calls
 * `useIdleBlock(true)` holds the slideshow off; a block that appears while idle wakes it.
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
  const [phase, setPhase] = useState<IdlePhase>("awake")
  const [blocked, setBlocked] = useState(false)
  const blockers = useRef(new Set<symbol>())
  const [memory] = useState(slideMemory)
  const screen = useRef<HTMLDivElement>(null)
  const returnFocus = useRef<Element | null>(null)
  const { timeoutMs, preview } = settings
  const showing = phase !== "awake"

  const block = useCallback(() => {
    const token = Symbol("idle block")
    blockers.current.add(token)
    setBlocked(true)
    // Declared busy or conversation activity takes the room back at once.
    setPhase("awake")

    return () => {
      blockers.current.delete(token)
      setBlocked(blockers.current.size > 0)
    }
  }, [])

  // The countdown starts afresh whenever the dashboard wakes or its last block is released.
  useEffect(() => {
    if (phase !== "awake" || blocked || timeoutMs === null) return
    let last = Date.now()
    let timer: ReturnType<typeof setTimeout> | undefined

    const touched = (): void => {
      last = Date.now()
    }

    function check(): void {
      const left = last + (timeoutMs ?? 0) - Date.now()

      if (left <= 0) setPhase("idle")
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
  }, [phase, blocked, timeoutMs])

  // A synthetic preview may ask for the slideshow straight away.
  useEffect(() => {
    if (!wantsIdleNow({ timeoutMs, preview }, window.location.search)) return

    const timer = setTimeout(() => {
      if (blockers.current.size === 0) setPhase("idle")
    }, 0)

    return () => clearTimeout(timer)
  }, [timeoutMs, preview])

  // Focus moves onto the slideshow while it shows and returns to where it was.
  useEffect(() => {
    if (phase === "awake") {
      const target = returnFocus.current
      returnFocus.current = null

      if (target instanceof HTMLElement && target.isConnected)
        target.focus({ preventScroll: true })

      return
    }

    if (phase === "idle") {
      returnFocus.current ??= document.activeElement
      screen.current?.focus({ preventScroll: true })
    }
  }, [phase])

  // While the slideshow shows, the first touch or key only wakes, and nothing of it gets through.
  useEffect(() => {
    if (!showing) return
    let finish: ReturnType<typeof setTimeout> | undefined

    function settle(delay: number): void {
      clearTimeout(finish)
      finish = setTimeout(() => setPhase("awake"), delay)
    }

    function swallow(event: Event): void {
      if (event.cancelable) event.preventDefault()
      event.stopImmediatePropagation()
    }

    function wake(event: Event): void {
      swallow(event)
      setPhase("waking")
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

  const state = useMemo(() => ({ phase, block }), [phase, block])

  return (
    <IdleContext value={state}>
      {children}
      {showing ? (
        <IdleScreen
          ref={screen}
          waking={phase === "waking"}
          memory={memory}
          overlay={overlay}
          sources={sources}
        />
      ) : null}
    </IdleContext>
  )
}
