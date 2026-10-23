import { z } from "zod"

const brightness = z.number().int().min(0).max(100)

/** A requested setting, independent of the controller's observable state. */
export const backlightSelection = z.discriminatedUnion("mode", [
  z.object({ mode: z.literal("off") }).strict(),
  z
    .object({ mode: z.literal("tv-sync"), brightness: brightness.optional() })
    .strict(),
  z
    .object({
      mode: z.literal("color"),
      rgb: z
        .object({
          red: z.number().int().min(0).max(255),
          green: z.number().int().min(0).max(255),
          blue: z.number().int().min(0).max(255),
        })
        .strict(),
      brightness,
    })
    .strict(),
])

/** The durable manual target sent through the private OpenHome transport. */
export type BacklightSelection = z.infer<typeof backlightSelection>

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
  bedtime: z.object({
    enabled: z.boolean(),
    start: z.string().regex(/^([01]\d|2[0-3]):[0-5]\d$/),
    end: z.string().regex(/^([01]\d|2[0-3]):[0-5]\d$/),
    brightness,
  }),
  control_source: z.enum(["automatic", "manual"]),
  override_expires_at: z.string().datetime().nullable(),
  next_transition: z.string().datetime().nullable(),
})

export const backlightAction = z.discriminatedUnion("action", [
  z.object({ action: z.literal("select"), selection: backlightSelection }),
  z.object({
    action: z.literal("profile"),
    source: z.enum(["capture", "backup"]),
  }),
  z.object({ action: z.literal("refresh") }),
  z.object({ action: z.literal("automatic") }),
  z.object({ action: z.literal("bedtime"), enabled: z.boolean() }),
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
