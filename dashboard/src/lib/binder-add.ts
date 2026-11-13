import { z } from "zod"
import { binderEntry, cardId, type BinderFailure } from "./binder"

/** One variant of a Card Code as search lists it: what is shown and what an addition names. */
export const cardVariant = z.object({
  id: cardId,
  card_code: z.string(),
  name: z.string(),
})

export type CardVariant = z.infer<typeof cardVariant>

export const cardSearchResponse = z.object({
  card_code: z.string(),
  cards: cardVariant.array(),
})

/** One deliberate catalogue lookup; Axum owns the Card Code format. */
export const cardSearchInput = z.object({
  card_code: z.string().trim().min(1).max(32),
})

/** An unlisted code is a successful, empty search; an outage is not an empty one. */
export type CardSearchResult =
  | { state: "found"; cards: CardVariant[] }
  | { state: "unlisted" }
  | { state: "invalid" }
  | { state: "outage" }
  | { state: "failed" }

const quantity = z.number().int().min(1)

/** What the household is about to add; its `request_id` is minted once and never reused. */
export const draft = z.object({
  owner_id: z.uuid(),
  card: cardVariant,
  quantity,
  language: binderEntry.shape.language,
  request_id: z.uuid(),
})

export type Draft = z.infer<typeof draft>

/** The wire command, with Axum looking up the Card itself. */
export const binderAddition = draft
  .omit({ card: true })
  .extend({ card_id: cardId })

export type BinderAddition = z.infer<typeof binderAddition>

export function additionOf(value: Draft): BinderAddition {
  return {
    owner_id: value.owner_id,
    card_id: value.card.id,
    quantity: value.quantity,
    language: value.language,
    request_id: value.request_id,
  }
}

export type AddFailure =
  | BinderFailure
  | "invalid"
  | "conflict"
  | "not-found"
  | "catalogue"

export type BinderAddReceipt =
  | { state: "saved" }
  | { state: "failed"; reason: AddFailure }

export const binderAddFailureMessage: Record<AddFailure, string> = {
  "missing-configuration":
    "Connect OpenHome in Connection before adding cards.",
  "invalid-configuration": "Check the private OpenHome setup in Connection.",
  "rejected-credential": "OpenHome rejected the API key. Check Connection.",
  unreachable:
    "The response was lost. Retry add: the same request will not add the card twice.",
  unavailable:
    "OpenHome could not confirm the result. Retry add: the same request will not add the card twice.",
  invalid: "Check the quantity and language, then retry the addition.",
  conflict:
    "This request conflicts with a saved change. Refresh to check the Binder, then discard the draft.",
  "not-found":
    "This member or Card is no longer available. Discard the draft and search again.",
  catalogue:
    "The card catalogue is not answering. Retry add in a moment; nothing was added.",
}

/**
 * The add sheet's whole lifecycle. Only `drafting` and `unconfirmed` outlive a refresh: a draft
 * keeps its owner, Card, quantity, language and `request_id` until a receipt or "Discard draft".
 * Once an addition has been sent its fields are locked, because a lost response may have been
 * saved and the same `request_id` must replay exactly the same addition.
 */
export type AddFlow =
  | { state: "idle" }
  | { state: "searching"; code: string }
  | { state: "found"; code: string; cards: CardVariant[] }
  | { state: "unlisted"; code: string }
  | { state: "invalid"; code: string }
  | { state: "outage"; code: string }
  | { state: "failed"; code: string }
  | { state: "drafting"; draft: Draft }
  | { state: "unconfirmed"; draft: Draft }
  | { state: "added"; draft: Draft }

export type AddStep =
  | { type: "search"; code: string }
  | { type: "searched"; code: string; result: CardSearchResult }
  | { type: "pick"; card: CardVariant; owner_id: string; request_id: string }
  | {
      type: "edit"
      change: Partial<Pick<Draft, "owner_id" | "quantity" | "language">>
    }
  | { type: "sent" }
  | { type: "saved" }
  | { type: "discard" }

/** The only place an AddFlow is constructed; a step that does not follow the state is ignored. */
export function advance(flow: AddFlow, step: AddStep): AddFlow {
  switch (step.type) {
    case "search":
      return flow.state === "drafting" ||
        flow.state === "unconfirmed" ||
        flow.state === "added"
        ? flow
        : { state: "searching", code: step.code }
    case "searched": {
      if (flow.state !== "searching" || flow.code !== step.code) return flow

      const { code, result } = step

      return result.state === "found"
        ? { state: "found", code, cards: result.cards }
        : { state: result.state, code }
    }

    case "pick":
      return flow.state === "found"
        ? {
            state: "drafting",
            draft: {
              owner_id: step.owner_id,
              card: step.card,
              quantity: 1,
              language: "en",
              request_id: step.request_id,
            },
          }
        : flow
    case "edit":
      return flow.state === "drafting"
        ? {
            state: "drafting",
            draft: {
              ...flow.draft,
              ...step.change,
              quantity: Math.max(
                1,
                step.change.quantity ?? flow.draft.quantity
              ),
            },
          }
        : flow
    case "sent":
      return flow.state === "drafting" || flow.state === "unconfirmed"
        ? { state: "unconfirmed", draft: flow.draft }
        : flow
    case "saved":
      return flow.state === "unconfirmed"
        ? { state: "added", draft: flow.draft }
        : flow
    case "discard":
      return flow.state === "drafting" ||
        flow.state === "unconfirmed" ||
        flow.state === "added"
        ? { state: "idle" }
        : flow
  }
}

const pending = z.object({
  state: z.enum(["drafting", "unconfirmed"]),
  draft,
})

const key = "openhome.pending-binder-addition"

/** The flow a refresh resumes: the stored draft, or a fresh sheet. */
export function recover(): AddFlow {
  try {
    const parsed = pending.safeParse(
      JSON.parse(sessionStorage.getItem(key) ?? "null")
    )

    if (parsed.success) return parsed.data
  } catch {
    /* Private browsing may disable storage. */
  }

  return { state: "idle" }
}

/** Keep the draft before any dispatch, and remove it only once the flow has left it. */
export function persist(flow: AddFlow): void {
  try {
    if (flow.state === "drafting" || flow.state === "unconfirmed")
      sessionStorage.setItem(
        key,
        JSON.stringify({ state: flow.state, draft: flow.draft })
      )
    else sessionStorage.removeItem(key)
  } catch {
    /* The mounted flow keeps its in-memory retry identity. */
  }
}
