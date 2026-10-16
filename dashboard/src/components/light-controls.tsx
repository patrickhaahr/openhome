import { Link } from "@tanstack/react-router"
import { ArrowUpRightIcon } from "@phosphor-icons/react/dist/csr/ArrowUpRight"
import { LightbulbIcon } from "@phosphor-icons/react/dist/csr/Lightbulb"
import { LightbulbFilamentIcon } from "@phosphor-icons/react/dist/csr/LightbulbFilament"
import { CheckAgainButton } from "@/components/check-again-button"
import { ControlAlert, OpenConnection } from "@/components/control-alert"
import { Button } from "@/components/ui/button"
import {
  connectionPausedMessages,
  type ControlLayout,
  failureTag,
  isSetupFailure,
  keyOutcome,
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

const keys = [
  { command: "on", label: "Turn on", Icon: LightbulbFilamentIcon },
  { command: "off", label: "Turn off", Icon: LightbulbIcon },
] as const

/**
 * Two keys, a lit bulb and a dark one. The press answers on the key itself; the sentence
 * stays in the live region. OpenHome never reports the lights' power, so nothing on screen
 * claims it.
 */
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

  const quick = layout === "quick"
  const disabled = pending || !connected

  return (
    <div
      className={quick ? "dir-row control-row" : "panel"}
      data-device="lights"
      role="group"
      aria-labelledby="lights-heading"
    >
      <div className={quick ? "control-row-name" : "panel-head"}>
        <h2 id="lights-heading" className={quick ? "dir-name" : "panel-title"}>
          Lights
        </h2>
        {failure === undefined ? null : (
          <ControlAlert tag={failureTag(connected, failure)} />
        )}
      </div>
      <div
        className={quick ? "control-row-actions" : "panel-keys"}
        role="group"
        aria-label="Light commands"
        aria-busy={pending}
      >
        {connected ? null : <CheckAgainButton compact />}
        {!connected || (failure !== undefined && isSetupFailure(failure)) ? (
          <OpenConnection />
        ) : null}
        {keys.map(({ command, label, Icon }) => (
          <Button
            key={command}
            size={quick ? "key-icon" : "key-hero"}
            variant={command === "on" ? "default" : "outline"}
            disabled={disabled}
            data-command={command}
            data-outcome={keyOutcome(commandState, command)}
            title={label}
            onClick={() => void send(command)}
          >
            <Icon
              size={quick ? 22 : 40}
              weight={command === "on" ? "fill" : "regular"}
              aria-hidden="true"
            />
            <span className="sr-only">{label}</span>
          </Button>
        ))}
      </div>
      {quick ? (
        <Link to="/controls" className="dir-go" title="All controls">
          <ArrowUpRightIcon size={18} aria-hidden="true" />
          <span className="sr-only">All controls</span>
        </Link>
      ) : null}
      <div className="sr-only">
        <p>
          Observed state <span>Unknown</span>
        </p>
        <p role="status" aria-live="polite" aria-atomic="true">
          {feedback}
        </p>
        <p>
          Commands control the configured lights. OpenHome does not report their
          current power state.
        </p>
      </div>
    </div>
  )
}
