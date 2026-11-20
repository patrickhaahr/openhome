import { z } from "zod"

export const speakerCommand = z.enum([
  "power",
  "bluetooth",
  "optical",
  "volume-down",
  "mute",
  "volume-up",
])

export type SpeakerCommand = z.infer<typeof speakerCommand>

export const speakerCommandLabels: Record<SpeakerCommand, string> = {
  power: "Power",
  bluetooth: "Bluetooth",
  optical: "Optical",
  "volume-down": "Volume down",
  mute: "Mute",
  "volume-up": "Volume up",
}
