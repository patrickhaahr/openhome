import { z } from "zod"
import { binderEntry, binderFailureMessage, type BinderFailure } from "./binder"

/** Quantity is stored as an unsigned 32-bit count. */
export const maxQuantity = 4294967295

/** An edit names only what changes: an absent field is kept and a null note clears it. */
export const binderCommand = z.discriminatedUnion("action", [
  z.object({
    action: z.literal("edit"),
    owner_id: z.uuid(),
    entry_id: z.uuid(),
    quantity: z.number().int().min(1).max(maxQuantity).optional(),
    language: binderEntry.shape.language.optional(),
    note: z.string().trim().min(1).max(500).nullable().optional(),
  }),
  z.object({
    action: z.literal("remove"),
    owner_id: z.uuid(),
    entry_id: z.uuid(),
  }),
])

/** A write always carries its selected owner. */
export type BinderCommand = z.infer<typeof binderCommand>

export type BinderWriteFailure =
  | BinderFailure
  | "invalid"
  | "conflict"
  | "not-found"

/** Confirmation of persistence, separate from uncertain network outcomes. */
export type BinderMutation =
  | { state: "saved" }
  | { state: "failed"; reason: BinderWriteFailure }

/** Actionable copy without upstream error bodies. */
export const binderWriteMessage: Record<BinderWriteFailure, string> = {
  ...binderFailureMessage,
  unreachable:
    "The response was lost. Refresh to check whether the change saved.",
  unavailable:
    "OpenHome could not confirm the result. Refresh before trying again.",
  invalid:
    "Check the quantity, a whole number of at least 1, and a note of up to 500 characters.",
  conflict:
    "This Binder already holds the card in that language. Raise that entry’s quantity and remove this one instead.",
  "not-found":
    "This card is no longer in the Binder. Refresh to see the current Binder.",
}
