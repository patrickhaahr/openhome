import { Link } from "@tanstack/react-router"
import { ArrowUpRightIcon } from "@phosphor-icons/react/dist/csr/ArrowUpRight"
import { LightbulbIcon } from "@phosphor-icons/react/dist/csr/Lightbulb"
import { CheckAgainButton } from "@/components/check-again-button"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader } from "@/components/ui/card"
import {
  connectionPausedMessages,
  type ControlLayout,
  isSetupFailure,
  setupFailureMessages,
} from "@/lib/command-feedback"
import type { ConnectionState } from "@/lib/connection"
import type {
  LightCommand,
  LightCommandResult,
  LightFailure,
} from "@/lib/lights"
import { useSingleCommand } from "@/lib/use-single-command"

type LightControlsProps = {
  connectionState: ConnectionState
  /** Only Home links to the focused Controls view. */
  layout: ControlLayout
  sendCommand: (options: {
    data: LightCommand
    signal: AbortSignal
  }) => Promise<LightCommandResult>
}

const commandLabels: Record<LightCommand, string> = { on: "On", off: "Off" }

const connectionMessages = connectionPausedMessages("light")

const failureMessages: Record<LightFailure, string> = {
  ...setupFailureMessages("light"),
  "missing-device":
    "Lights are not configured. Ask the installer to set up SwitchBot in OpenHome, then try again.",
  unreachable:
    "No acknowledgement received. The lights may have changed. Check them and your connection before trying again.",
  unavailable:
    "Command not confirmed. Check the lights and OpenHome’s SwitchBot connection before trying again.",
}

/** Light commands and their acknowledgements stay separate from unknown observed power. */
export function LightControls({
  layout,
  connectionState,
  sendCommand,
}: LightControlsProps): React.JSX.Element {
  const connected = connectionState === "connected"

  const { commandState, send } = useSingleCommand({
    sendCommand,
    ready: connected,
  })

  const pending = commandState.state === "pending"
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
          {!connected || (failure !== undefined && isSetupFailure(failure)) ? (
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
