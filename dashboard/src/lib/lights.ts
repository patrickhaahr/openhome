import { z } from "zod"
import type { ConnectionState } from "./connection"

/** The Light Commands Axum supports for the configured SwitchBot lights. */
export const lightCommand = z.enum(["on", "off"])

/** One Light Command; sending it never establishes observed light state. */
export type LightCommand = z.infer<typeof lightCommand>

/** Sanitized reasons OpenHome did not confirm a Light Command. */
export type LightFailure =
  | Exclude<ConnectionState, "connected">
  | "missing-device"

/** Whether OpenHome acknowledged a Light Command, safe to serialize. */
export type LightCommandResult =
  | { state: "accepted" }
  | { state: "failed"; reason: LightFailure }
