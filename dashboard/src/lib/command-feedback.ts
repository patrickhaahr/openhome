import type { ConnectionState } from "./connection"

/** Home shows a control's common commands; the focused Controls view shows all of them. */
export type ControlLayout = "quick" | "full"

/** Failures that only a corrected dashboard connection can fix. */
export type SetupFailure = Exclude<
  ConnectionState,
  "connected" | "unreachable" | "unavailable"
>

/** The device a control surface commands, as it reads in copy. */
export type Device = "light" | "TV" | "speaker"

function capitalized(device: Device): string {
  return `${device.charAt(0).toUpperCase()}${device.slice(1)}`
}

/**
 * Copy for a failed connection check, shown before any command is sent.
 *
 * @param device - The commanded device.
 * @returns Messages that never suggest the device changed.
 */
export function connectionPausedMessages(
  device: Device
): Record<Exclude<ConnectionState, "connected">, string> {
  const commands = `${capitalized(device)} commands`

  return {
    "missing-configuration": `Connect OpenHome before sending a ${device} command.`,
    "invalid-configuration": `The OpenHome connection needs a correction before ${device} commands can be sent.`,
    "rejected-credential": `OpenHome rejected the API key. ${commands} are paused until it is fixed.`,
    unreachable: `OpenHome is unreachable. ${commands} are paused until it responds.`,
    unavailable: `OpenHome is not responding normally. ${commands} are paused until it recovers.`,
  }
}

/**
 * Copy for a command that revealed a dashboard setup problem.
 *
 * @param device - The commanded device.
 * @returns Messages pointing to Connection for setup help.
 */
export function setupFailureMessages(
  device: Device
): Record<SetupFailure, string> {
  return {
    "missing-configuration": `Connect OpenHome before sending a ${device} command.`,
    "invalid-configuration":
      "The OpenHome connection needs a correction. Open Connection for setup help.",
    "rejected-credential":
      "OpenHome rejected the API key. Open Connection for setup help.",
  }
}

const setupFailures = new Set<string>([
  "missing-configuration",
  "invalid-configuration",
  "rejected-credential",
])

/** Whether a failure needs the Connection setup page. */
export function isSetupFailure(reason: string): reason is SetupFailure {
  return setupFailures.has(reason)
}

/** How the last-pressed key shows its outcome; every other key stays at rest. */
export type KeyOutcome = "pending" | "accepted" | "failed"

/**
 * The outcome to draw on one key: a press, then a flash for an acknowledgement or a shake for
 * a failure. An acknowledgement is never drawn as device state.
 *
 * @param state - The surface's latest command state.
 * @param command - The key's command.
 * @returns The key's outcome, or undefined for keys that were not pressed last.
 */
export function keyOutcome<C extends string>(
  state:
    | { state: "idle" }
    | { state: "pending"; command: C }
    | { state: "result"; command: C; result: { state: string } },
  command: C
): KeyOutcome | undefined {
  if (state.state === "idle" || state.command !== command) return undefined

  if (state.state === "pending") return "pending"

  return state.result.state === "accepted" ? "accepted" : "failed"
}

const connectionTags: Record<Exclude<ConnectionState, "connected">, string> = {
  "missing-configuration": "Not set up",
  "invalid-configuration": "Check setup",
  "rejected-credential": "Key rejected",
  unreachable: "Offline",
  unavailable: "Not responding",
}

const commandTags = {
  "missing-configuration": "Not set up",
  "invalid-configuration": "Check setup",
  "rejected-credential": "Key rejected",
  "missing-device": "Not set up",
  "unsupported-command": "Key gone",
  unreachable: "No answer",
  unavailable: "Not confirmed",
} as const satisfies Record<string, string>

/** Every failure a control can show: its connection's, or its last command's. */
export type FailureReason = keyof typeof commandTags

function isConnectionFailure(
  reason: FailureReason
): reason is Exclude<ConnectionState, "connected"> {
  return reason in connectionTags
}

/**
 * One or two visible words for a failure; the full sentence stays in the live region.
 *
 * @param connected - Whether the failure came from a command rather than the connection.
 * @param reason - The failure reason.
 * @returns A terse tag, never a claim about the device.
 */
export function failureTag(connected: boolean, reason: FailureReason): string {
  return !connected && isConnectionFailure(reason)
    ? connectionTags[reason]
    : commandTags[reason]
}
