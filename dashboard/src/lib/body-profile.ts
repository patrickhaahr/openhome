import { z } from "zod"

/** The existing singleton Profile; null means the value is not configured. */
export const profile = z.object({
  height_cm: z.number().nullable(),
  sex: z.string().nullable(),
  hr_max_bpm: z.number().int().nullable(),
  /** `YYYY-MM-DD`; nutrition targets need it for BMR. */
  birth_date: z.iso.date().nullable(),
})

/** Shared fitness configuration as served by Axum. */
export type Profile = z.infer<typeof profile>

/** A daily Body Weight record, kept separately from Workouts. */
export const bodyWeight = z.object({
  id: z.number().int(),
  date: z.iso.date(),
  weight_kg: z.number(),
})

/** Body Weight history in Axum's chronological order. */
export type BodyWeight = z.infer<typeof bodyWeight>

/** Optional calendar bounds for the authenticated history read. */
export const bodyRange = z.object({
  from: z.iso.date().optional().catch(undefined),
  to: z.iso.date().optional().catch(undefined),
})

/** Date filters retained in the Workout URL. */
export type BodyRange = z.infer<typeof bodyRange>

/** Supported writes; omitted Profile fields preserve values, explicit null clears them. */
export const bodyCommand = z.discriminatedUnion("kind", [
  profile.partial().extend({ kind: z.literal("profile") }),
  z.object({
    kind: z.literal("weight"),
    date: z.string(),
    weight_kg: z.number(),
  }),
])

/** One shared Profile patch or new daily Body Weight record. */
export type BodyCommand = z.infer<typeof bodyCommand>

/** Sanitized failures; no private upstream diagnostics reach the display. */
export type BodyFailure =
  | "missing-configuration"
  | "invalid-configuration"
  | "rejected-credential"
  | "unreachable"
  | "unavailable"
  | "invalid"
  | "duplicate-date"

/** A complete read, distinguished from empty or unconfigured values. */
export type BodyState =
  | {
      readonly state: "available"
      readonly profile: Profile
      readonly history: BodyWeight[]
    }
  | { readonly state: "unavailable"; readonly reason: BodyFailure }

/** Persistence is confirmed only by a parsed Axum receipt. */
export type BodyMutation =
  | { readonly state: "saved" }
  | { readonly state: "failed"; readonly reason: BodyFailure }

/** Static, actionable copy for failures. */
export const bodyFailureMessage: Record<BodyFailure, string> = {
  "missing-configuration":
    "Connect OpenHome in Connection to read your profile and body weight.",
  "invalid-configuration": "Check the private OpenHome setup in Connection.",
  "rejected-credential": "OpenHome rejected the API key. Check Connection.",
  unreachable:
    "The response was lost. Reload to check whether the change saved before trying again.",
  unavailable:
    "OpenHome could not confirm the result. Reload before trying again.",
  invalid:
    "OpenHome rejected the values. Check the date, positive height or weight, a whole maximum heart rate from 100 to 240 bpm, and a birth date that is not in the future.",
  "duplicate-date":
    "Body weight is already recorded for this date. Reload to review it, or choose another date. The existing record has not been replaced.",
}

/** Editable Profile fields, preserving exact stored number text. */
type ProfileDraft = {
  height: string
  sex: string
  heartRate: string
  birthDate: string
}

/** Text inputs retain exact stored numbers until a user changes them. */
export function profileDraft(value: Profile): ProfileDraft {
  return {
    height: value.height_cm === null ? "" : String(value.height_cm),
    sex: value.sex ?? "",
    heartRate: value.hr_max_bpm?.toString() ?? "",
    birthDate: value.birth_date ?? "",
  }
}

/** Accept decimal commas on the household keyboard; blank optional numbers clear a field. */
export function optionalNumber(value: string): number | null {
  const trimmed = value.trim().replace(",", ".")

  if (trimmed === "") return null

  return /^\d+(\.\d+)?$/.test(trimmed) ? Number(trimmed) : Number.NaN
}

/** Only edited fields are sent, so an unrelated concurrent Profile change is preserved. */
export function profilePatch(
  initial: ReturnType<typeof profileDraft>,
  draft: ReturnType<typeof profileDraft>
): Extract<BodyCommand, { kind: "profile" }> {
  const patch: Extract<BodyCommand, { kind: "profile" }> = { kind: "profile" }

  if (draft.height !== initial.height)
    patch.height_cm = optionalNumber(draft.height)

  if (draft.sex !== initial.sex) patch.sex = draft.sex === "" ? null : draft.sex

  if (draft.heartRate !== initial.heartRate)
    patch.hr_max_bpm = optionalNumber(draft.heartRate)

  if (draft.birthDate !== initial.birthDate)
    patch.birth_date = draft.birthDate === "" ? null : draft.birthDate

  return patch
}
