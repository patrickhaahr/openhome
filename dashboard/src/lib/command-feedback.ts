import type { ConnectionState } from "./connection"

export type ControlLayout = "quick" | "full"

export type SetupFailure = Exclude<
  ConnectionState,
  "connected" | "unreachable" | "unavailable"
>

export type Device = "light" | "TV" | "speaker"

function capitalized(device: Device): string {
  return `${device.charAt(0).toUpperCase()}${device.slice(1)}`
}

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

export function isSetupFailure(reason: string): reason is SetupFailure {
  return setupFailures.has(reason)
}

export type KeyOutcome = "pending" | "accepted" | "failed"

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

export type FailureReason = keyof typeof commandTags

function isConnectionFailure(
  reason: FailureReason
): reason is Exclude<ConnectionState, "connected"> {
  return reason in connectionTags
}

export function failureTag(connected: boolean, reason: FailureReason): string {
  return !connected && isConnectionFailure(reason)
    ? connectionTags[reason]
    : commandTags[reason]
}
