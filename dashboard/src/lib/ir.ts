import type { ConnectionState } from "./connection"
import type { SpeakerCommand } from "./speaker"
import type { TvCommand } from "./tv"

/** Sanitized reasons OpenHome could not report or confirm an IR Remote's Commands. */
export type IrFailure =
  | Exclude<ConnectionState, "connected">
  | "missing-device"
  | "unsupported-command"

/** Why OpenHome could not report an IR Remote's Commands; a stale command cannot apply. */
export type DiscoveryFailure = Exclude<IrFailure, "unsupported-command">

/** The Commands OpenHome currently reports for one IR Remote, safe to serialize. */
export type IrRemoteState<C extends string> =
  | { state: "available"; commands: C[] }
  | { state: "unavailable"; reason: DiscoveryFailure }

/** Whether OpenHome acknowledged a Command; it never establishes device state. Safe to serialize. */
export type IrCommandResult =
  | { state: "accepted" }
  | { state: "failed"; reason: IrFailure }

/** Both IR Remotes come from one discovery, so they always share its outcome. */
export type IrRemotes = {
  tv: IrRemoteState<TvCommand>
  speaker: IrRemoteState<SpeakerCommand>
}

/**
 * Give every IR Remote the same discovery failure; one request discovers them all.
 *
 * @param reason - Why OpenHome could not report the remotes' Commands.
 * @returns An unavailable state for each remote.
 */
export function unavailableRemotes(reason: DiscoveryFailure): IrRemotes {
  return {
    tv: { state: "unavailable", reason },
    speaker: { state: "unavailable", reason },
  }
}
