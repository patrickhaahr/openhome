import { useState } from "react"
import { Await } from "@tanstack/react-router"
import type { Icon } from "@phosphor-icons/react/dist/lib/types"
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
  type Device,
  failureTag,
  isSetupFailure,
  keyOutcome,
  setupFailureMessages,
} from "@/lib/command-feedback"
import type { ConnectionState } from "@/lib/connection"
import type {
  DiscoveryFailure,
  IrCommandResult,
  IrFailure,
  IrRemoteState,
} from "@/lib/ir"
import { cn } from "@/lib/utils"
import { type SingleCommand, useSingleCommand } from "@/lib/use-single-command"

/** A labelled set of Remote Buttons. */
export type CommandGroup<C extends string> = {
  label: string
  commands: C[]
}

/** A group on the full remote, with where it sits and how its keys are arranged. */
export type FullGroup<C extends string> = CommandGroup<C> & {
  /** Classes for the group's place among the others. */
  place: string
  /** Classes for the touch layout of its keys. */
  keys: string
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
  /** Home's glyph for a Command when its own icon would not say which device it drives. */
  plateIcons: Partial<Record<C, Icon>>
  /** Whether Home's plate shows the device glyph; keys that name the device need none. */
  plateIdentity: boolean
  /** Commands whose glyph keeps its word on the full remote, where the glyph alone is unclear. */
  worded: readonly C[]
  quickGroups: CommandGroup<C>[]
  fullGroups: FullGroup<C>[]
  /** Extra classes for Home's plate. */
  plate: string
  /** The full panel's shape: radius and padding. */
  panel: string
  /** The full remote's arrangement of its groups. */
  groups: string
  /** Extra classes for the device glyph on the full panel. */
  identity: string
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
  /** Where the full panel sits on its page. */
  className?: string | undefined
}

const quickButtons =
  "flex flex-col flex-wrap items-center gap-2 max-phone:flex-row"

const recheckFailures = new Set<IrFailure>([
  "missing-device",
  "unsupported-command",
])

/** Copy for one remote's discovery, connection and command outcomes. */
type RemoteMessages = {
  discovery: Record<DiscoveryFailure, string>
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
        <RemoteCard
          config={config}
          layout={props.layout}
          className={props.className}
          keys={
            <span
              className="flex gap-2 *:size-12 *:rounded-full *:bg-wash"
              aria-hidden="true"
            >
              <span />
              <span />
              <span />
              <span />
            </span>
          }
          status={
            <p role="status" aria-live="polite">
              Checking which {config.device} commands OpenHome reports…
            </p>
          }
        />
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
  className,
}: Pick<IrRemoteProps<C>, "connectionState" | "layout" | "className"> & {
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

  const groups = (
    layout === "quick"
      ? config.quickGroups.map((group) => ({
          ...group,
          place: "contents",
          keys: quickButtons,
        }))
      : config.fullGroups
  )
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

  const quick = layout === "quick"

  const status = (
    <p role="status" aria-live="polite" aria-atomic="true">
      {feedback}
    </p>
  )

  const recovery = (
    <>
      {!connected ||
      discoveryFailed ||
      (failure !== undefined && recheckFailures.has(failure)) ? (
        <CheckAgainButton compact />
      ) : null}
      {!connected || (failure !== undefined && isSetupFailure(failure)) ? (
        <OpenConnection />
      ) : null}
    </>
  )

  const keys =
    groups.length > 0 ? (
      <div className={quick ? "contents" : config.groups} aria-busy={pending}>
        {groups.map((group) => (
          <div
            key={group.label}
            className={group.place}
            data-slot="remote-group"
            role="group"
            aria-label={group.label}
          >
            {quick ? null : <h3 className="sr-only">{group.label}</h3>}
            <div className={group.keys}>
              {group.commands.map((command) => (
                <RemoteButton
                  key={command}
                  config={config}
                  command={command}
                  compact={quick}
                  outcome={keyOutcome(commandState, command)}
                  disabled={pending || !connected}
                  onPress={(command) => void send(command)}
                />
              ))}
            </div>
          </div>
        ))}
      </div>
    ) : null

  const tag =
    failure !== undefined
      ? failureTag(connected, failure)
      : remote.state === "available" && groups.length === 0
        ? "No keys"
        : undefined

  return (
    <RemoteCard
      config={config}
      layout={layout}
      className={className}
      alert={
        tag === undefined ? null : (
          <ControlAlert
            tag={tag}
            className={quick ? alertOnPlate : undefined}
          />
        )
      }
      recovery={recovery}
      keys={keys}
      status={status}
    />
  )
}

function RemoteCard<C extends string>({
  config,
  layout,
  className,
  alert,
  recovery,
  keys,
  status,
}: {
  config: IrRemoteConfig<C>
  layout: ControlLayout
  className?: string | undefined
  alert?: React.ReactNode
  recovery?: React.ReactNode
  keys?: React.ReactNode
  status: React.ReactNode
}): React.JSX.Element {
  const headingId = `${config.id}-heading`
  const quick = layout === "quick"

  return (
    <ControlPanel
      layout={layout}
      className={quick ? config.plate : cn(config.panel, className)}
      labelledBy={headingId}
    >
      <PanelHead layout={layout}>
        {/* The device's glyph names the remote; the word stays for assistive technology. */}
        {!quick || config.plateIdentity ? (
          <config.icon
            className={
              quick
                ? "mt-2.5 mb-1 text-muted-ink"
                : `text-muted-ink ${config.identity}`
            }
            size={quick ? 30 : 32}
            aria-hidden="true"
          />
        ) : null}
        <h2 id={headingId} className="sr-only">
          {config.title}
        </h2>
        {alert}
      </PanelHead>
      {quick ? (
        <div className="flex flex-col items-center gap-2 max-phone:flex-row">
          {recovery}
          {keys}
        </div>
      ) : (
        <>
          {recovery === undefined ? null : (
            <div className="flex items-center justify-center gap-2">
              {recovery}
            </div>
          )}
          {keys}
        </>
      )}
      <div className="sr-only">
        {quick ? null : <p>{config.remoteName}</p>}
        <p>
          {config.unobserved} <span>Not reported</span>
        </p>
        {status}
        <p>{config.explanation}</p>
      </div>
    </ControlPanel>
  )
}

function RemoteButton<C extends string>({
  config,
  command,
  compact,
  outcome,
  disabled,
  onPress,
}: {
  config: IrRemoteConfig<C>
  command: C
  /** Home shows icon keys; the label stays the button's text for assistive technology. */
  compact: boolean
  outcome: ReturnType<typeof keyOutcome>
  disabled: boolean
  onPress: (command: C) => void
}): React.JSX.Element {
  // Indexing a generic record does not resolve to a JSX component type without this annotation.
  const CommandIcon: Icon | undefined =
    (compact ? config.plateIcons[command] : undefined) ??
    config.commandIcons[command]

  const label = config.labels[command]

  const iconOnly =
    CommandIcon !== undefined && (compact || !config.worded.includes(command))

  return (
    <Button
      size={compact || iconOnly ? "key-round" : "key"}
      variant={command === config.primary ? "default" : "outline"}
      disabled={disabled}
      data-command={command}
      data-icon-only={iconOnly}
      data-outcome={outcome}
      title={iconOnly ? label : undefined}
      onClick={() => onPress(command)}
    >
      {CommandIcon === undefined ? null : (
        <CommandIcon size={compact ? 26 : 20} aria-hidden="true" />
      )}
      {iconOnly ? (
        <span className="sr-only">{label}</span>
      ) : (
        <span>{label}</span>
      )}
    </Button>
  )
}
