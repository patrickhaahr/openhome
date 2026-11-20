import type {
  BacklightAction,
  BacklightResult,
  BacklightSelection,
} from "@/lib/backlight"

export const modeLabel = {
  off: "Off",
  "tv-sync": "TV Sync",
  color: "Color",
  effect: "Effect",
  other: "Other",
}

export const commandLabel = {
  idle: "No command",
  pending: "Pending",
  acknowledged: "Acknowledged",
  failed: "Failed; selection saved",
}

export const recoveryLabel = {
  idle: "No recovery needed",
  pending: "Pending",
  retrying: "Retrying latest selection",
  recovered: "Current selection recovered",
  "profile-required": "TV Sync settings needed",
}

export const connectionLabel = {
  unconfigured: "Setup needed",
  unknown: "Not checked",
  reachable: "Reachable",
  unreachable: "Unreachable",
}

export const unconfirmed =
  "OpenHome couldn’t confirm the command. The LED may have changed; refresh before trying again."

export type Debounce = {
  readonly kind: "debounce"
  readonly selection: BacklightSelection
  readonly timer: ReturnType<typeof setTimeout>
}

export type Waiting =
  | Debounce
  | { readonly kind: "queued"; readonly action: BacklightAction }

export type Delivery =
  | { readonly phase: "idle" }
  | { readonly phase: "debouncing"; readonly debounce: Debounce }
  | {
      readonly phase: "sending"
      readonly run: number
      readonly action: BacklightAction
      readonly next: Waiting | null
    }
  | { readonly phase: "closed" }

export function discard(waiting: Waiting | null): void {
  if (waiting?.kind === "debounce") clearTimeout(waiting.timer)
}

type Announcement = {
  readonly text: string
  readonly tag: string | null
}

export function describe(
  action: BacklightAction,
  result: BacklightResult
): Announcement {
  switch (result.state) {
    case "available":
      if (result.status.command === "failed")
        return {
          text: "OpenHome couldn’t confirm the command. Your selection is saved; OpenHome retries the latest selection automatically.",
          tag: "Retrying",
        }

      if (action.action === "profile")
        return { text: "TV Sync settings saved safely.", tag: null }

      if (action.action === "bedtime")
        return {
          text: "Bedtime settings saved. Readable device state is listed with the Backlight state.",
          tag: null,
        }

      if (action.action === "refresh")
        return result.status.reachability === "reachable"
          ? { text: "Backlight state refreshed.", tag: null }
          : { text: "The Backlight couldn’t be reached.", tag: "No answer" }

      if (result.status.command === "pending")
        return {
          text: "Your selection is saved and waiting for the Backlight.",
          tag: null,
        }

      return {
        text: "Command acknowledged. Readable device state is listed with the Backlight state.",
        tag: null,
      }
    case "profile-required":
      return {
        text: "TV Sync settings are needed. Activate TV Sync on the controller and capture, or recover the private backup.",
        tag: "Setup needed",
      }
    case "invalid-profile":
      return {
        text: "The saved profile is invalid. Your previous TV Sync settings were kept.",
        tag: "Not saved",
      }
    case "invalid-settings":
      return {
        text: "Bedtime settings weren’t saved. Check the times and red brightness, then try again.",
        tag: "Not saved",
      }
    case "unavailable":
      return { text: unconfirmed, tag: "No answer" }
  }
}
