import { z } from "zod"

export const profile = z.object({
  height_cm: z.number().nullable(),
  sex: z.string().nullable(),
  hr_max_bpm: z.number().int().nullable(),
  birth_date: z.iso.date().nullable(),
})

export type Profile = z.infer<typeof profile>

export const bodyWeight = z.object({
  id: z.number().int(),
  date: z.iso.date(),
  weight_kg: z.number(),
})

export type BodyWeight = z.infer<typeof bodyWeight>

export const bodyRange = z.object({
  from: z.iso.date().optional().catch(undefined),
  to: z.iso.date().optional().catch(undefined),
})

export type BodyRange = z.infer<typeof bodyRange>

export const bodyCommand = z.discriminatedUnion("kind", [
  profile.partial().extend({ kind: z.literal("profile") }),
  z.object({
    kind: z.literal("weight"),
    date: z.string(),
    weight_kg: z.number(),
  }),
])

export type BodyCommand = z.infer<typeof bodyCommand>

export type BodyFailure =
  | "missing-configuration"
  | "invalid-configuration"
  | "rejected-credential"
  | "unreachable"
  | "unavailable"
  | "invalid"
  | "duplicate-date"

export type BodyState =
  | {
      readonly state: "available"
      readonly profile: Profile
      readonly history: BodyWeight[]
    }
  | { readonly state: "unavailable"; readonly reason: BodyFailure }

export type BodyMutation =
  | { readonly state: "saved" }
  | { readonly state: "failed"; readonly reason: BodyFailure }

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

export const profileSexes = [
  { value: "male", label: "Male" },
  { value: "female", label: "Female" },
] as const

export function profileSex(value: string | null): string {
  if (value === null) return ""
  const canonical = value.trim().toLowerCase()

  return profileSexes.some((sex) => sex.value === canonical) ? canonical : value
}

type ProfileDraft = {
  height: string
  sex: string
  heartRate: string
  birthDate: string
}

export function profileDraft(value: Profile): ProfileDraft {
  return {
    height: value.height_cm === null ? "" : String(value.height_cm),
    sex: profileSex(value.sex),
    heartRate: value.hr_max_bpm?.toString() ?? "",
    birthDate: value.birth_date ?? "",
  }
}

export function optionalNumber(value: string): number | null {
  const trimmed = value.trim().replace(",", ".")

  if (trimmed === "") return null

  return /^\d+(\.\d+)?$/.test(trimmed) ? Number(trimmed) : Number.NaN
}

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
