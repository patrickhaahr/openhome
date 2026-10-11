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
