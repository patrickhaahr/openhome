import { z } from "zod"

const brightness = z.number().int().min(0).max(100)

const clockTime = z
  .string()
  .regex(/^([01]\d|2[0-3]):[0-5]\d$/, "Use HH:MM from 00:00 to 23:59.")

/** Editable window fields, also checked by the authoritative API. */
export const bedtimeFields = z
  .object({
    start: clockTime,
    end: clockTime,
    brightness,
  })
  .refine((value) => value.start !== value.end, {
    message: "Start and End must be different.",
    path: ["end"],
  })

const effectId = z.enum(["forest", "aurora", "rainbow", "fire", "party"])

/** A requested setting, independent of the controller's observable state. */
export const backlightSelection = z.discriminatedUnion("mode", [
  z.object({ mode: z.literal("off") }).strict(),
  z
    .object({ mode: z.literal("effect"), effect: effectId, brightness })
    .strict(),
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
  effects: z
    .array(
      z.object({ id: effectId, name: z.string(), description: z.string() })
    )
    .default([]),
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
  recovery: z
    .enum(["idle", "pending", "retrying", "recovered", "profile-required"])
    .default("idle"),
  bedtime: z.object({
    enabled: z.boolean(),
    start: clockTime,
    end: clockTime,
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
  z
    .object({
      action: z.literal("bedtime"),
      enabled: z.boolean().optional(),
      start: clockTime.optional(),
      end: clockTime.optional(),
      brightness: brightness.optional(),
    })
    .refine(
      (value) =>
        value.start === undefined ||
        value.end === undefined ||
        value.start !== value.end,
      {
        message: "Start and End must be different.",
      }
    ),
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
  | { state: "invalid-settings" }
