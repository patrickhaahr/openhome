import { createContext, use, useEffect } from "react"

export type IdlePhase = "awake" | "idle" | "waking"

type IdleState = {
  readonly phase: IdlePhase
  readonly block: () => () => void
}

export const IdleContext = createContext<IdleState | null>(null)

export function useIdlePhase(): IdlePhase {
  return use(IdleContext)?.phase ?? "awake"
}

export function useIdleBlock(active: boolean): void {
  const block = use(IdleContext)?.block

  useEffect(() => {
    if (!active || block === undefined) return

    return block()
  }, [active, block])
}
