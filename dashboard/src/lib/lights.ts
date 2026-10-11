import { z } from "zod"
import type { ConnectionState } from "./connection"

export const lightCommand = z.enum(["on", "off"])

export type LightCommand = z.infer<typeof lightCommand>

export type LightFailure =
  | Exclude<ConnectionState, "connected">
  | "missing-device"

export type LightCommandResult =
  | { state: "accepted" }
  | { state: "failed"; reason: LightFailure }
