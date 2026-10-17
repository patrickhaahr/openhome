import { createContext, use, useEffect } from "react"

/**
 * awake: the dashboard as it was left. idle: the slideshow covers it. waking: the first touch or
 * key landed; the slideshow fades but stays on top, swallowing the rest of that gesture.
 */
export type IdlePhase = "awake" | "idle" | "waking"

type IdleState = {
  readonly phase: IdlePhase
  /** Register one reason not to go idle; the returned function releases it. */
  readonly block: () => () => void
}

/** Provided by IdleProvider in the shell; null outside it (tests, isolated components). */
export const IdleContext = createContext<IdleState | null>(null)

/** Whether the Idle Slideshow covers the dashboard; awake outside a provider. */
export function useIdlePhase(): IdlePhase {
  return use(IdleContext)?.phase ?? "awake"
}

/**
 * Holds the Idle Slideshow off while `active`: an open sheet or dialog, an unsaved draft, a
 * save in flight or a conversation. Outside an IdleProvider it does nothing.
 */
export function useIdleBlock(active: boolean): void {
  const block = use(IdleContext)?.block

  useEffect(() => {
    if (!active || block === undefined) return

    return block()
  }, [active, block])
}
