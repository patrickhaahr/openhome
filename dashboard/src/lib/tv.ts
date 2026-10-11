import { z } from "zod"
import type { ConnectionState } from "./connection"

/** LG TV Commands the dashboard has Remote Buttons for; OpenHome reports which are available. */
export const tvCommand = z.enum([
  "power",
  "input",
  "volume-up",
  "volume-down",
  "mute",
  "home",
  "settings",
  "up",
  "down",
  "left",
  "right",
  "ok",
  "back",
  "exit",
  "channel-up",
  "channel-down",
  "guide",
  "info",
  "0",
  "1",
  "2",
  "3",
  "4",
  "5",
  "6",
  "7",
  "8",
  "9",
  "hdmi-1",
  "hdmi-2",
  "hdmi-3",
])

/** One LG TV Command; an acknowledgement never establishes the TV's power or volume. */
export type TvCommand = z.infer<typeof tvCommand>

/** Remote Button labels, phrased as actions rather than observed TV state. */
export const tvCommandLabels: Record<TvCommand, string> = {
  power: "Power",
  input: "Input",
  "volume-up": "Volume up",
  "volume-down": "Volume down",
  mute: "Mute",
  home: "Home",
  settings: "Settings",
  up: "Up",
  down: "Down",
  left: "Left",
  right: "Right",
  ok: "OK",
  back: "Back",
  exit: "Exit",
  "channel-up": "Channel up",
  "channel-down": "Channel down",
  guide: "Guide",
  info: "Info",
  "0": "0",
  "1": "1",
  "2": "2",
  "3": "3",
  "4": "4",
  "5": "5",
  "6": "6",
  "7": "7",
  "8": "8",
  "9": "9",
  "hdmi-1": "HDMI 1",
  "hdmi-2": "HDMI 2",
  "hdmi-3": "HDMI 3",
}

/** Sanitized reasons OpenHome could not report or confirm TV commands. */
export type TvFailure =
  | Exclude<ConnectionState, "connected">
  | "missing-device"
  | "unsupported-command"

/** The TV commands OpenHome currently reports, safe to serialize. */
export type TvRemoteState =
  | { state: "available"; commands: TvCommand[] }
  | {
      state: "unavailable"
      reason: Exclude<TvFailure, "unsupported-command">
    }

/** Whether OpenHome acknowledged a TV Command, safe to serialize. */
export type TvCommandResult =
  | { state: "accepted" }
  | { state: "failed"; reason: TvFailure }
