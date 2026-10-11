import { useState } from "react"
import { Await, Link } from "@tanstack/react-router"
import { ArrowUpRightIcon } from "@phosphor-icons/react/dist/csr/ArrowUpRight"
import { CaretDownIcon } from "@phosphor-icons/react/dist/csr/CaretDown"
import { CaretLeftIcon } from "@phosphor-icons/react/dist/csr/CaretLeft"
import { CaretRightIcon } from "@phosphor-icons/react/dist/csr/CaretRight"
import { CaretUpIcon } from "@phosphor-icons/react/dist/csr/CaretUp"
import { MinusIcon } from "@phosphor-icons/react/dist/csr/Minus"
import { PlusIcon } from "@phosphor-icons/react/dist/csr/Plus"
import { PowerIcon } from "@phosphor-icons/react/dist/csr/Power"
import { SpeakerSimpleSlashIcon } from "@phosphor-icons/react/dist/csr/SpeakerSimpleSlash"
import { TelevisionSimpleIcon } from "@phosphor-icons/react/dist/csr/TelevisionSimple"
import type { Icon } from "@phosphor-icons/react/dist/lib/types"
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
import {
  tvCommandLabels,
  type TvCommand,
  type TvCommandResult,
  type TvFailure,
  type TvRemoteState,
} from "@/lib/tv"
import { type SingleCommand, useSingleCommand } from "@/lib/use-single-command"

type SendTvCommand = (options: {
  data: TvCommand
  signal: AbortSignal
}) => Promise<TvCommandResult>

type TvRemoteProps = {
  connectionState: ConnectionState
  /** Deferred discovery, so a slow IR remote never delays the rest of the page. */
  remote: Promise<TvRemoteState>
  sendCommand: SendTvCommand
  layout: ControlLayout
}

type CommandGroup = {
  label: string
  kind: "quick" | "row" | "dpad" | "keypad"
  commands: TvCommand[]
}

const quickGroups: CommandGroup[] = [
  {
    label: "Common TV commands",
    kind: "quick",
    commands: ["power", "mute", "volume-down", "volume-up"],
  },
]

const fullGroups: CommandGroup[] = [
  {
    label: "Power and input",
    kind: "row",
    commands: ["power", "input", "hdmi-1", "hdmi-2", "hdmi-3"],
  },
  {
    label: "Volume",
    kind: "row",
    commands: ["volume-down", "mute", "volume-up"],
  },
  {
    label: "Navigation",
    kind: "dpad",
    commands: ["up", "left", "ok", "right", "down"],
  },
  {
    label: "Menus",
    kind: "row",
    commands: ["back", "home", "exit", "settings", "info", "guide"],
  },
  { label: "Channels", kind: "row", commands: ["channel-down", "channel-up"] },
  {
    label: "Numbers",
    kind: "keypad",
    commands: ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"],
  },
]

const commandIcons: Partial<Record<TvCommand, Icon>> = {
  power: PowerIcon,
  mute: SpeakerSimpleSlashIcon,
  "volume-down": MinusIcon,
  "volume-up": PlusIcon,
  up: CaretUpIcon,
  down: CaretDownIcon,
  left: CaretLeftIcon,
  right: CaretRightIcon,
}

const connectionMessages = connectionPausedMessages("TV")

const missingDevice =
  "The TV remote is not configured. Ask the installer to set up the IR remote in OpenHome, then check again."

/** Discovery failures; no command has been sent, so the TV cannot have changed. */
const discoveryMessages: Record<
  Extract<TvRemoteState, { state: "unavailable" }>["reason"],
  string
> = {
  ...connectionMessages,
  "missing-device": missingDevice,
  unreachable:
    "OpenHome did not report the TV commands in time. Check your connection, then check again.",
  unavailable:
    "OpenHome could not list the TV commands. Check its IR remote, then check again.",
}

const failureMessages: Record<TvFailure, string> = {
  ...setupFailureMessages("TV"),
  "missing-device": missingDevice,
  "unsupported-command":
    "The IR remote no longer reports this command. Check again to refresh the available buttons.",
  unreachable:
    "No acknowledgement received. The TV may have responded. Check it and your connection before trying again.",
  unavailable:
    "Command not confirmed. Check the TV and OpenHome’s IR remote before trying again.",
}

const recheckFailures = new Set<TvFailure>([
  "missing-device",
  "unsupported-command",
])

/** An LG TV remote whose buttons come from the commands OpenHome reports. */
export function TvRemote({
  remote,
  sendCommand,
  ...props
}: TvRemoteProps): React.JSX.Element {
  // Held above Await so a re-check that suspends discovery cannot reset the pending-command guard.
  const command = useSingleCommand({
    sendCommand,
    ready: props.connectionState === "connected",
  })

  return (
    <Await
      promise={remote}
      fallback={
        <TvRemoteCard layout={props.layout}>
          <p className="control-feedback" role="status" aria-live="polite">
            Checking which TV commands OpenHome reports…
          </p>
        </TvRemoteCard>
      }
    >
      {(discovered) => (
        <TvRemoteControls remote={discovered} command={command} {...props} />
      )}
    </Await>
  )
}

function TvRemoteControls({
  connectionState,
  remote,
  command: { commandState, send, clear },
  layout,
}: Pick<TvRemoteProps, "connectionState" | "layout"> & {
  remote: TvRemoteState
  command: SingleCommand<TvCommand, TvFailure>
}): React.JSX.Element {
  const [discovery, setDiscovery] = useState(remote)

  // Each check resolves a new discovery; an earlier outcome no longer describes these buttons.
  if (discovery !== remote) {
    setDiscovery(remote)
    clear()
  }

  const pending = commandState.state === "pending"
  const connected = connectionState === "connected"
  const available = new Set(remote.state === "available" ? remote.commands : [])

  const groups = (layout === "quick" ? quickGroups : fullGroups)
    .map((group) => ({
      ...group,
      commands: group.commands.filter((command) => available.has(command)),
    }))
    .filter((group) => group.commands.length > 0)

  let feedback = "Ready to send a command."
  let failure: TvFailure | undefined

  if (commandState.state === "pending") {
    feedback = `Sending ${tvCommandLabels[commandState.command]} command…`
  } else if (!connected) {
    feedback = connectionMessages[connectionState]
    failure = connectionState
  } else if (remote.state === "unavailable") {
    feedback = discoveryMessages[remote.reason]
    failure = remote.reason
  } else if (commandState.state === "result") {
    const { result, command } = commandState

    if (result.state === "accepted") {
      feedback = `${tvCommandLabels[command]} command accepted by OpenHome.`
    } else {
      feedback = failureMessages[result.reason]
      failure = result.reason
    }
  } else if (available.size === 0) {
    feedback = "OpenHome reports no TV commands this remote supports."
  } else if (groups.length === 0) {
    feedback = "OpenHome reports no common TV commands. Open the full remote."
  }

  const discoveryFailed = remote.state === "unavailable"

  const status = (
    <p
      className="control-feedback"
      role="status"
      aria-live="polite"
      aria-atomic="true"
      data-failed={failure !== undefined}
    >
      {feedback}
    </p>
  )

  return (
    <TvRemoteCard layout={layout}>
      {/* The full remote is long; keep its outcome beside the commands it describes. */}
      {layout === "full" ? status : null}
      {groups.length > 0 ? (
        <div className="tv-groups" aria-busy={pending}>
          {groups.map((group) => (
            <div
              key={group.label}
              className="tv-group"
              role="group"
              aria-label={group.label}
            >
              {layout === "full" ? (
                <h3 className="page-eyebrow">{group.label}</h3>
              ) : null}
              <div className="tv-buttons" data-kind={group.kind}>
                {group.commands.map((command) => (
                  <TvButton
                    key={command}
                    command={command}
                    disabled={pending || !connected}
                    onPress={(command) => void send(command)}
                  />
                ))}
              </div>
            </div>
          ))}
        </div>
      ) : null}
      {layout === "quick" ? status : null}
      <p className="control-explanation">
        Commands are sent to the TV by infrared. OpenHome does not report
        whether the TV is on, its input or its volume.
      </p>
      <div className="control-links">
        {!connected ||
        discoveryFailed ||
        (failure !== undefined && recheckFailures.has(failure)) ? (
          <CheckAgainButton />
        ) : null}
        {!connected || (failure !== undefined && isSetupFailure(failure)) ? (
          <Link to="/connection" className="action-link">
            Open Connection
          </Link>
        ) : null}
        {layout === "quick" && !discoveryFailed ? (
          <Link to="/controls" className="action-link">
            Full remote <ArrowUpRightIcon size={18} aria-hidden="true" />
          </Link>
        ) : null}
      </div>
    </TvRemoteCard>
  )
}

function TvRemoteCard({
  layout,
  children,
}: {
  layout: TvRemoteProps["layout"]
  children: React.ReactNode
}): React.JSX.Element {
  return (
    <Card className="h-full" role="group" aria-labelledby="tv-heading">
      <CardHeader>
        <div className="feature-card-top">
          <TelevisionSimpleIcon size={28} aria-hidden="true" />
          <span className="page-eyebrow">
            {layout === "quick" ? "Home controls" : "LG TV remote"}
          </span>
        </div>
        <h2 id="tv-heading" className="feature-card-title">
          TV
        </h2>
        <p className="control-observed">
          Power and volume <span>Not reported</span>
        </p>
      </CardHeader>
      <CardContent>{children}</CardContent>
    </Card>
  )
}

function TvButton({
  command,
  disabled,
  onPress,
}: {
  command: TvCommand
  disabled: boolean
  onPress: (command: TvCommand) => void
}): React.JSX.Element {
  const CommandIcon = commandIcons[command]

  return (
    <Button
      size="lg"
      variant={command === "power" ? "default" : "outline"}
      disabled={disabled}
      data-command={command}
      onClick={() => onPress(command)}
    >
      {CommandIcon === undefined ? null : (
        <CommandIcon size={20} aria-hidden="true" />
      )}
      {tvCommandLabels[command]}
    </Button>
  )
}
