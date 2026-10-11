import { useRef, useState } from "react"
import { Link } from "@tanstack/react-router"
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
  sendCommand: (options: { data: LightCommand }) => Promise<LightCommandResult>
}

type CommandState =
  | { state: "idle" }
  | { state: "pending"; command: LightCommand }
  | { state: "result"; command: LightCommand; result: LightCommandResult }

const failures: Record<LightFailure, string> = {
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

/** Light commands and their acknowledgements stay separate from unknown observed power. */
export function LightControls({
  connectionState,
  sendCommand,
}: LightControlsProps): React.JSX.Element {
  const [commandState, setCommandState] = useState<CommandState>({
    state: "idle",
  })

  const inFlight = useRef(false)
  const pending = commandState.state === "pending"
  const connected = connectionState === "connected"
  let feedback = "Ready to send a command."
  let failed = false

  if (commandState.state === "pending") {
    feedback = `Sending ${commandState.command} command…`
  } else if (commandState.state === "result") {
    const { result, command } = commandState
    failed = result.state === "failed"
    feedback =
      result.state === "accepted"
        ? `${command === "on" ? "On" : "Off"} command accepted by OpenHome.`
        : failures[result.reason]
  } else if (!connected) {
    feedback = failures[connectionState]
    failed = true
  }

  async function send(command: LightCommand): Promise<void> {
    // The synchronous guard also catches taps queued before React renders disabled buttons.
    if (inFlight.current || !connected) return

    inFlight.current = true
    setCommandState({ state: "pending", command })

    try {
      const result = await sendCommand({ data: command })
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

  return (
    <Card className="h-full" aria-labelledby="lights-heading">
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
          data-failed={failed}
        >
          {feedback}
        </p>
        <p className="light-explanation">
          Commands control the configured lights. OpenHome does not report their
          current power state.
        </p>
        {!connected ? (
          <Link to="/connection" className="action-link">
            Open Connection
          </Link>
        ) : null}
      </CardContent>
    </Card>
  )
}
