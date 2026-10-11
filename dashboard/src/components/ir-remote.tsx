import { useState } from "react"
import { Await, Link } from "@tanstack/react-router"
import { ArrowUpRightIcon } from "@phosphor-icons/react/dist/csr/ArrowUpRight"
import type { Icon } from "@phosphor-icons/react/dist/lib/types"
import { CheckAgainButton } from "@/components/check-again-button"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader } from "@/components/ui/card"
import {
  connectionPausedMessages,
  type ControlLayout,
  type Device,
  isSetupFailure,
  setupFailureMessages,
} from "@/lib/command-feedback"
import type { ConnectionState } from "@/lib/connection"
import type { IrCommandResult, IrFailure, IrRemoteState } from "@/lib/ir"
import { type SingleCommand, useSingleCommand } from "@/lib/use-single-command"

/** A labelled set of Remote Buttons; `kind` selects its touch layout. */
export type CommandGroup<C extends string> = {
  label: string
  kind: "quick" | "trio" | "row" | "dpad" | "keypad"
  commands: C[]
}

/** What distinguishes one Dashboard Remote from another. */
export type IrRemoteConfig<C extends string> = {
  /** Prefixes element ids, so several remotes can share a page. */
  id: string
  device: Extract<Device, "TV" | "speaker">
  title: string
  /** Names the remote on the focused Controls view. */
  remoteName: string
  icon: Icon
  /** The Command given the filled button style. */
  primary: C
  labels: Record<C, string>
  commandIcons: Partial<Record<C, Icon>>
  quickGroups: CommandGroup<C>[]
  fullGroups: CommandGroup<C>[]
  /** The device state OpenHome never reports, shown as "Not reported". */
  unobserved: string
  explanation: string
}

/** Props every Dashboard Remote takes from its page. */
export type IrRemoteProps<C extends string> = {
  connectionState: ConnectionState
  /** Deferred discovery, so a slow IR remote never delays the rest of the page. */
  remote: Promise<IrRemoteState<C>>
  sendCommand: (options: {
    data: C
    signal: AbortSignal
  }) => Promise<IrCommandResult>
  layout: ControlLayout
}

const recheckFailures = new Set<IrFailure>([
  "missing-device",
  "unsupported-command",
])

/** Copy for one remote's discovery, connection and command outcomes. */
type RemoteMessages = {
  discovery: Record<
    Extract<IrRemoteState<string>, { state: "unavailable" }>["reason"],
    string
  >
  failure: Record<IrFailure, string>
  connection: ReturnType<typeof connectionPausedMessages>
}

/** Copy for one remote; no message suggests the device changed. */
function remoteMessages(
  device: IrRemoteConfig<string>["device"]
): RemoteMessages {
  const connection = connectionPausedMessages(device)

  const missingDevice = `The ${device} remote is not configured. Ask the installer to set up the IR remote in OpenHome, then check again.`

  return {
    connection,
    // No command has been sent, so the device cannot have changed.
    discovery: {
      ...connection,
      "missing-device": missingDevice,
      unreachable: `OpenHome did not report the ${device} commands in time. Check your connection, then check again.`,
      unavailable: `OpenHome could not list the ${device} commands. Check its IR remote, then check again.`,
    },
    failure: {
      ...setupFailureMessages(device),
      "missing-device": missingDevice,
      "unsupported-command":
        "The IR remote no longer reports this command. Check again to refresh the available buttons.",
      unreachable: `No acknowledgement received. The ${device} may have responded. Check it and your connection before trying again.`,
      unavailable: `Command not confirmed. Check the ${device} and OpenHome’s IR remote before trying again.`,
    },
  }
}

/** A Dashboard Remote whose buttons come from the Commands OpenHome reports. */
export function IrRemote<C extends string>({
  config,
  remote,
  sendCommand,
  ...props
}: IrRemoteProps<C> & { config: IrRemoteConfig<C> }): React.JSX.Element {
  // Held above Await so a re-check that suspends discovery cannot reset the pending-command guard.
  const command = useSingleCommand({
    sendCommand,
    ready: props.connectionState === "connected",
  })

  return (
    <Await
      promise={remote}
      fallback={
        <RemoteCard config={config} layout={props.layout}>
          <p className="control-feedback" role="status" aria-live="polite">
            Checking which {config.device} commands OpenHome reports…
          </p>
        </RemoteCard>
      }
    >
      {(discovered) => (
        <RemoteControls
          config={config}
          remote={discovered}
          command={command}
          {...props}
        />
      )}
    </Await>
  )
}

function RemoteControls<C extends string>({
  config,
  connectionState,
  remote,
  command: { commandState, send, clear },
  layout,
}: Pick<IrRemoteProps<C>, "connectionState" | "layout"> & {
  config: IrRemoteConfig<C>
  remote: IrRemoteState<C>
  command: SingleCommand<C, IrFailure>
}): React.JSX.Element {
  const [discovery, setDiscovery] = useState(remote)

  // Each check resolves a new discovery; an earlier outcome no longer describes these buttons.
  if (discovery !== remote) {
    setDiscovery(remote)
    clear()
  }

  const { device, labels } = config
  const messages = remoteMessages(device)
  const pending = commandState.state === "pending"
  const connected = connectionState === "connected"
  const available = new Set(remote.state === "available" ? remote.commands : [])

  const groups = (layout === "quick" ? config.quickGroups : config.fullGroups)
    .map((group) => ({
      ...group,
      commands: group.commands.filter((command) => available.has(command)),
    }))
    .filter((group) => group.commands.length > 0)

  let feedback = "Ready to send a command."
  let failure: IrFailure | undefined

  if (commandState.state === "pending") {
    feedback = `Sending ${labels[commandState.command]} command…`
  } else if (!connected) {
    feedback = messages.connection[connectionState]
    failure = connectionState
  } else if (remote.state === "unavailable") {
    feedback = messages.discovery[remote.reason]
    failure = remote.reason
  } else if (commandState.state === "result") {
    const { result, command } = commandState

    if (result.state === "accepted") {
      feedback = `${labels[command]} command accepted by OpenHome.`
    } else {
      feedback = messages.failure[result.reason]
      failure = result.reason
    }
  } else if (available.size === 0) {
    feedback = `OpenHome reports no ${device} commands this remote supports.`
  } else if (groups.length === 0) {
    feedback = `OpenHome reports no common ${device} commands. Open the full remote.`
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
    <RemoteCard config={config} layout={layout}>
      {/* The full remote is long; keep its outcome beside the commands it describes. */}
      {layout === "full" ? status : null}
      {groups.length > 0 ? (
        <div className="remote-groups" aria-busy={pending}>
          {groups.map((group) => (
            <div
              key={group.label}
              className="remote-group"
              role="group"
              aria-label={group.label}
            >
              {layout === "full" ? (
                <h3 className="page-eyebrow">{group.label}</h3>
              ) : null}
              <div className="remote-buttons" data-kind={group.kind}>
                {group.commands.map((command) => (
                  <RemoteButton
                    key={command}
                    config={config}
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
      <p className="control-explanation">{config.explanation}</p>
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
    </RemoteCard>
  )
}

function RemoteCard<C extends string>({
  config,
  layout,
  children,
}: {
  config: IrRemoteConfig<C>
  layout: ControlLayout
  children: React.ReactNode
}): React.JSX.Element {
  const headingId = `${config.id}-heading`

  return (
    <Card className="h-full" role="group" aria-labelledby={headingId}>
      <CardHeader>
        <div className="feature-card-top">
          <config.icon size={28} aria-hidden="true" />
          <span className="page-eyebrow">
            {layout === "quick" ? "Home controls" : config.remoteName}
          </span>
        </div>
        <h2 id={headingId} className="feature-card-title">
          {config.title}
        </h2>
        <p className="control-observed">
          {config.unobserved} <span>Not reported</span>
        </p>
      </CardHeader>
      <CardContent>{children}</CardContent>
    </Card>
  )
}

function RemoteButton<C extends string>({
  config,
  command,
  disabled,
  onPress,
}: {
  config: IrRemoteConfig<C>
  command: C
  disabled: boolean
  onPress: (command: C) => void
}): React.JSX.Element {
  const CommandIcon: Icon | undefined = config.commandIcons[command]

  return (
    <Button
      size="lg"
      variant={command === config.primary ? "default" : "outline"}
      disabled={disabled}
      data-command={command}
      onClick={() => onPress(command)}
    >
      {CommandIcon === undefined ? null : (
        <CommandIcon size={20} aria-hidden="true" />
      )}
      {config.labels[command]}
    </Button>
  )
}
