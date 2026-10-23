import { z } from "zod"

export const backlightSelection = z.object({ mode: z.enum(["off", "tv-sync"]) })

export const backlightStatus = z.object({
  setup: z.enum(["incomplete", "ready"]),
  reachability: z.enum(["unconfigured", "unknown", "reachable", "unreachable"]),
  intended: backlightSelection,
  acknowledged: backlightSelection.nullable(),
  observed: z
    .object({
      power: z.boolean(),
      brightness: z.number().int().min(0).max(100),
      mode: z.enum(["tv-sync", "color", "other"]),
      observed_at: z.string().datetime(),
    })
    .nullable(),
  stale: z.boolean(),
  command: z.enum(["idle", "pending", "acknowledged", "failed"]),
})

export const backlightAction = z.discriminatedUnion("action", [
  z.object({ action: z.literal("select"), selection: backlightSelection }),
  z.object({
    action: z.literal("profile"),
    source: z.enum(["capture", "backup"]),
  }),
  z.object({ action: z.literal("refresh") }),
])

export type BacklightAction = z.infer<typeof backlightAction>

export type BacklightStatus = z.infer<typeof backlightStatus>

export type BacklightRead =
  | { state: "available"; status: BacklightStatus }
  | { state: "unavailable" }

export type BacklightResult =
  | BacklightRead
  | { state: "profile-required" }
  | { state: "invalid-profile" }
