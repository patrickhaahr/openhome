import { useRef, useState } from "react"

export type Acknowledgement<F extends string> =
  | { state: "accepted" }
  | { state: "failed"; reason: F }

export type CommandState<C extends string, F extends string> =
  | { state: "idle" }
  | { state: "pending"; command: C }
  | {
      state: "result"
      command: C
      result: Acknowledgement<F | "unreachable">
    }

export type SingleCommand<C extends string, F extends string> = {
  commandState: CommandState<C, F>
  send: (command: C) => Promise<void>
  clear: () => void
}

const commandTimeoutMs = 15000

export function useSingleCommand<C extends string, F extends string>({
  sendCommand,
  ready,
}: {
  sendCommand: (options: {
    data: C
    signal: AbortSignal
  }) => Promise<Acknowledgement<F>>
  ready: boolean
}): SingleCommand<C, F> {
  const [commandState, setCommandState] = useState<CommandState<C, F>>({
    state: "idle",
  })

  const inFlight = useRef(false)

  async function send(command: C): Promise<void> {
    if (inFlight.current || !ready) return

    inFlight.current = true
    setCommandState({ state: "pending", command })

    try {
      const result = await sendCommand({
        data: command,
        signal: AbortSignal.timeout(commandTimeoutMs),
      })

      setCommandState({ state: "result", command, result })
    } catch {
      setCommandState({
        state: "result",
        command,
        result: { state: "failed", reason: "unreachable" },
      })
    } finally {
      inFlight.current = false
    }
  }

  function clear(): void {
    if (!inFlight.current) setCommandState({ state: "idle" })
  }

  return { commandState, send, clear }
}
