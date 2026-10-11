import { useRef, useState } from "react"

/** OpenHome's sanitized answer to one command. */
export type Acknowledgement<F extends string> =
  | { state: "accepted" }
  | { state: "failed"; reason: F }

/** The latest command a control surface sent and what OpenHome answered. */
export type CommandState<C extends string, F extends string> =
  | { state: "idle" }
  | { state: "pending"; command: C }
  | {
      state: "result"
      command: C
      result: Acknowledgement<F | "unreachable">
    }

/** A control surface's command state and its burst-safe sender. */
export type SingleCommand<C extends string, F extends string> = {
  commandState: CommandState<C, F>
  /** Ignores taps while a command is pending or the surface is not ready. */
  send: (command: C) => Promise<void>
  /** Forget a settled outcome, for example after controls are rediscovered. */
  clear: () => void
}

// The dashboard server allows Axum 12 seconds; this also bounds a dropped iPad connection.
const commandTimeoutMs = 15000

/**
 * Send at most one command at a time and never retry it automatically.
 *
 * @param options.sendCommand - The server function that dispatches one command.
 * @param options.ready - Whether OpenHome is connected; taps are ignored otherwise.
 * @returns The latest command state and a burst-safe `send`.
 */
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
    // The synchronous guard also catches taps queued before React renders disabled buttons.
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
      // RPC failures may include private details. Offer a safe, uncertain outcome without retrying.
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
