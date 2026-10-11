import { useRef, useState, useTransition } from "react"
import { Link, useRouter } from "@tanstack/react-router"
import { ArrowUpRightIcon } from "@phosphor-icons/react/dist/csr/ArrowUpRight"
import { LightbulbIcon } from "@phosphor-icons/react/dist/csr/Lightbulb"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader } from "@/components/ui/card"
import type { ConnectionState } from "@/lib/connection"
import type {
  LightCommand,
  LightCommandResult,
  LightFailure,
} from "@/lib/lights"

type LightControlsProps = {
  connectionState: ConnectionState
  sendCommand: (options: {
    data: LightCommand
    signal: AbortSignal
  }) => Promise<LightCommandResult>
}

type CommandState =
  | { state: "idle" }
  | { state: "pending"; command: LightCommand }
  | { state: "result"; command: LightCommand; result: LightCommandResult }

// The dashboard server allows Axum 12 seconds; this also bounds a dropped iPad connection.
const commandTimeoutMs = 15000

const commandLabels: Record<LightCommand, string> = { on: "On", off: "Off" }

/** Shown before any command is sent, so the copy never suggests the lights changed. */
const connectionMessages: Record<
  Exclude<ConnectionState, "connected">,
  string
> = {
  "missing-configuration": "Connect OpenHome before sending a light command.",
  "invalid-configuration":
    "The OpenHome connection needs a correction before light commands can be sent.",
  "rejected-credential":
    "OpenHome rejected the API key. Light commands are paused until it is fixed.",
  unreachable:
    "OpenHome is unreachable. Light commands are paused until it responds.",
  unavailable:
    "OpenHome is not responding normally. Light commands are paused until it recovers.",
}

const failureMessages: Record<LightFailure, string> = {
  "missing-configuration": "Connect OpenHome before sending a light command.",
  "invalid-configuration":
    "The OpenHome connection needs a correction. Open Connection for setup help.",
  "rejected-credential":
    "OpenHome rejected the API key. Open Connection for setup help.",
  "missing-device":
    "Lights are not configured. Ask the installer to set up SwitchBot in OpenHome, then try again.",
  unreachable:
    "No acknowledgement received. The lights may have changed. Check them and your connection before trying again.",
  unavailable:
    "Command not confirmed. Check the lights and OpenHome’s SwitchBot connection before trying again.",
}

const setupFailures = new Set<LightFailure>([
  "missing-configuration",
  "invalid-configuration",
  "rejected-credential",
])

/** Light commands and their acknowledgements stay separate from unknown observed power. */
export function LightControls({
  connectionState,
  sendCommand,
}: LightControlsProps): React.JSX.Element {
  const router = useRouter()
  const [isChecking, startTransition] = useTransition()

  const [commandState, setCommandState] = useState<CommandState>({
    state: "idle",
  })

  const inFlight = useRef(false)
  const pending = commandState.state === "pending"
  const connected = connectionState === "connected"
  let feedback = "Ready to send a command."
  let failure: LightFailure | undefined

  if (commandState.state === "pending") {
    feedback = `Sending ${commandState.command} command…`
  } else if (!connected) {
    feedback = connectionMessages[connectionState]
    failure = connectionState
  } else if (commandState.state === "result") {
    const { result, command } = commandState

    if (result.state === "accepted") {
      feedback = `${commandLabels[command]} command accepted by OpenHome.`
    } else {
      feedback = failureMessages[result.reason]
      failure = result.reason
    }
  }

  async function send(command: LightCommand): Promise<void> {
    // The synchronous guard also catches taps queued before React renders disabled buttons.
    if (inFlight.current || !connected) return

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

  function checkAgain(): void {
    startTransition(async () => {
      await router.invalidate({ sync: true })
    })
  }

  return (
    <Card className="h-full" role="group" aria-labelledby="lights-heading">
      <CardHeader>
        <div className="feature-card-top">
          <LightbulbIcon size={28} aria-hidden="true" />
          <span className="page-eyebrow">Home controls</span>
        </div>
        <h2 id="lights-heading" className="feature-card-title">
          Lights
        </h2>
        <p className="light-observed">
          Observed state <span>Unknown</span>
        </p>
      </CardHeader>
      <CardContent>
        <div
          className="light-actions"
          role="group"
          aria-label="Light commands"
          aria-busy={pending}
        >
          <Button
            size="lg"
            disabled={pending || !connected}
            onClick={() => void send("on")}
          >
            Turn on
          </Button>
          <Button
            size="lg"
            variant="outline"
            disabled={pending || !connected}
            onClick={() => void send("off")}
          >
            Turn off
          </Button>
        </div>
        <p
          className="light-feedback"
          role="status"
          aria-live="polite"
          aria-atomic="true"
          data-failed={failure !== undefined}
        >
          {feedback}
        </p>
        <p className="light-explanation">
          Commands control the configured lights. OpenHome does not report their
          current power state.
        </p>
        <div className="light-links">
          {connected ? null : (
            <Button
              variant="outline"
              disabled={isChecking}
              onClick={checkAgain}
            >
              {isChecking ? "Checking…" : "Check again"}
            </Button>
          )}
          {!connected ||
          (failure !== undefined && setupFailures.has(failure)) ? (
            <Link to="/connection" className="action-link">
              Open Connection
            </Link>
          ) : null}
          <Link
            to="/$feature"
            params={{ feature: "controls" }}
            className="action-link"
          >
            All controls <ArrowUpRightIcon size={18} aria-hidden="true" />
          </Link>
        </div>
      </CardContent>
    </Card>
  )
}
