import { LightbulbIcon } from "@phosphor-icons/react/dist/csr/Lightbulb"
import { LightbulbFilamentIcon } from "@phosphor-icons/react/dist/csr/LightbulbFilament"
import { CheckAgainButton } from "@/components/check-again-button"
import { ControlAlert, OpenConnection } from "@/components/control-alert"
import {
  alertOnPlate,
  ControlPanel,
  PanelHead,
} from "@/components/control-panel"
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

const fullPanel = "flex-1 rounded-full p-2 max-phone:rounded-4xl"

const quickKeys = "flex flex-col items-center gap-2 max-phone:flex-row"

const fullKeys =
  "flex flex-1 flex-col items-center gap-2 self-stretch max-phone:h-28 max-phone:flex-none max-phone:flex-row max-phone:items-stretch **:data-[slot=button]:h-auto **:data-[slot=button]:w-full **:data-[slot=button]:flex-1 **:data-[slot=button]:[&_svg:not([class*=size-])]:size-12 **:data-[command=on]:rounded-t-key-pill **:data-[command=on]:rounded-b-tile **:data-[command=on]:[&_svg]:text-sun **:data-[command=off]:rounded-t-tile **:data-[command=off]:rounded-b-key-pill max-phone:**:data-[command=on]:rounded-l-key-pill max-phone:**:data-[command=on]:rounded-r-tile max-phone:**:data-[command=off]:rounded-l-tile max-phone:**:data-[command=off]:rounded-r-key-pill"

const keys = [
  { command: "on", label: "Turn on", Icon: LightbulbFilamentIcon },
  { command: "off", label: "Turn off", Icon: LightbulbIcon },
] as const

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
    <ControlPanel
      layout={layout}
      className={quick ? "max-phone:rounded-4xl" : fullPanel}
      labelledBy="lights-heading"
    >
      <PanelHead layout={layout} bare={failure === undefined}>
        <h2 id="lights-heading" className="sr-only">
          Lights
        </h2>
        {failure === undefined ? null : (
          <ControlAlert
            tag={failureTag(connected, failure)}
            className={quick ? alertOnPlate : undefined}
          />
        )}
      </PanelHead>
      <div
        className={quick ? quickKeys : fullKeys}
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
            size={quick ? "key-round" : "key-hero"}
            variant={command === "on" ? "default" : "outline"}
            disabled={disabled}
            data-command={command}
            data-outcome={keyOutcome(commandState, command)}
            title={label}
            onClick={() => void send(command)}
          >
            <Icon
              size={quick ? 28 : 40}
              weight={command === "on" ? "fill" : "regular"}
              aria-hidden="true"
            />
            <span className="sr-only">{label}</span>
          </Button>
        ))}
      </div>
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
    </ControlPanel>
  )
}
