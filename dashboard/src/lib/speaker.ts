import { z } from "zod"

/** Edifier speaker Commands the dashboard has Remote Buttons for; OpenHome reports which are available. */
export const speakerCommand = z.enum([
  "power",
  "bluetooth",
  "optical",
  "volume-down",
  "mute",
  "volume-up",
])

/** One Edifier speaker Command; an acknowledgement never establishes power, input or volume. */
export type SpeakerCommand = z.infer<typeof speakerCommand>

/** Remote Button labels, phrased as actions rather than observed speaker state. */
export const speakerCommandLabels: Record<SpeakerCommand, string> = {
  power: "Power",
  bluetooth: "Bluetooth",
  optical: "Optical",
  "volume-down": "Volume down",
  mute: "Mute",
  "volume-up": "Volume up",
}
