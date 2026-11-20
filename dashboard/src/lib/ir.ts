import type { ConnectionState } from "./connection"
import type { SpeakerCommand } from "./speaker"
import type { TvCommand } from "./tv"

export type IrFailure =
  | Exclude<ConnectionState, "connected">
  | "missing-device"
  | "unsupported-command"

export type DiscoveryFailure = Exclude<IrFailure, "unsupported-command">

export type IrRemoteState<C extends string> =
  | { state: "available"; commands: C[] }
  | { state: "unavailable"; reason: DiscoveryFailure }

export type IrCommandResult =
  | { state: "accepted" }
  | { state: "failed"; reason: IrFailure }

export type IrRemotes = {
  tv: IrRemoteState<TvCommand>
  speaker: IrRemoteState<SpeakerCommand>
}

export function unavailableRemotes(reason: DiscoveryFailure): IrRemotes {
  return {
    tv: { state: "unavailable", reason },
    speaker: { state: "unavailable", reason },
  }
}
