import type { ConnectionState } from "./connection"

/** Sanitized reasons OpenHome could not report or confirm an IR Remote's Commands. */
export type IrFailure =
  | Exclude<ConnectionState, "connected">
  | "missing-device"
  | "unsupported-command"

/** The Commands OpenHome currently reports for one IR Remote, safe to serialize. */
export type IrRemoteState<C extends string> =
  | { state: "available"; commands: C[] }
  | {
      state: "unavailable"
      reason: Exclude<IrFailure, "unsupported-command">
    }

/** Whether OpenHome acknowledged a Command; it never establishes device state. Safe to serialize. */
export type IrCommandResult =
  | { state: "accepted" }
  | { state: "failed"; reason: IrFailure }
