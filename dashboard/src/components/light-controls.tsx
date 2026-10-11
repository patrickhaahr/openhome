import { Link } from "@tanstack/react-router"
import { ArrowUpRightIcon } from "@phosphor-icons/react/dist/csr/ArrowUpRight"
import { LightbulbIcon } from "@phosphor-icons/react/dist/csr/Lightbulb"
import { CheckAgainButton } from "@/components/check-again-button"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader } from "@/components/ui/card"
import type { ConnectionState } from "@/lib/connection"
import type {
  LightCommand,
  LightCommandResult,
  LightFailure,
} from "@/lib/lights"
import { useSingleCommand } from "@/lib/use-single-command"

type LightControlsProps = {
  connectionState: ConnectionState
  /** Home links to the focused Controls view, which shows every control. */
  layout: "quick" | "full"
  sendCommand: (options: {
    data: LightCommand
    signal: AbortSignal
  }) => Promise<LightCommandResult>
}

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
  layout,
  connectionState,
  sendCommand,
}: LightControlsProps): React.JSX.Element {
  const { commandState, send } = useSingleCommand(sendCommand)
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

  function dispatch(command: LightCommand): void {
    if (connected) void send(command)
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
        <p className="control-observed">
          Observed state <span>Unknown</span>
        </p>
      </CardHeader>
      <CardContent>
        <div
          className="control-actions"
          role="group"
          aria-label="Light commands"
          aria-busy={pending}
        >
          <Button
            size="lg"
            disabled={pending || !connected}
            onClick={() => dispatch("on")}
          >
            Turn on
          </Button>
          <Button
            size="lg"
            variant="outline"
            disabled={pending || !connected}
            onClick={() => dispatch("off")}
          >
            Turn off
          </Button>
        </div>
        <p
          className="control-feedback"
          role="status"
          aria-live="polite"
          aria-atomic="true"
          data-failed={failure !== undefined}
        >
          {feedback}
        </p>
        <p className="control-explanation">
          Commands control the configured lights. OpenHome does not report their
          current power state.
        </p>
        <div className="control-links">
          {connected ? null : <CheckAgainButton />}
          {!connected ||
          (failure !== undefined && setupFailures.has(failure)) ? (
            <Link to="/connection" className="action-link">
              Open Connection
            </Link>
          ) : null}
          {layout === "quick" ? (
            <Link to="/controls" className="action-link">
              All controls <ArrowUpRightIcon size={18} aria-hidden="true" />
            </Link>
          ) : null}
        </div>
      </CardContent>
    </Card>
  )
}
