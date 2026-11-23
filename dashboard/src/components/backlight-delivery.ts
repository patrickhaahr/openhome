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
  readonly id: number
}

export type Waiting =
  | Debounce
  | { readonly kind: "queued"; readonly action: BacklightAction }

declare const deliveryBrand: unique symbol

type DeliveryState =
  | { readonly phase: "idle" }
  | { readonly phase: "debouncing"; readonly debounce: Debounce }
  | {
      readonly phase: "sending"
      readonly run: number
      readonly action: BacklightAction
      readonly next: Waiting | null
    }
  | { readonly phase: "closed" }

export type Delivery = DeliveryState & { readonly [deliveryBrand]: true }

export type DeliveryEvent =
  | {
      readonly type: "edit"
      readonly selection: BacklightSelection
      readonly id: number
    }
  | {
      readonly type: "perform"
      readonly action: BacklightAction
      readonly run: number
    }
  | { readonly type: "fire"; readonly id: number; readonly run: number }
  | { readonly type: "settle"; readonly run: number }
  | { readonly type: "close" }

export type DeliveryEffect =
  | { readonly kind: "cancel"; readonly id: number }
  | { readonly kind: "schedule"; readonly id: number }
  | {
      readonly kind: "send"
      readonly action: BacklightAction
      readonly run: number
    }

export type DeliveryStep = {
  readonly delivery: Delivery
  readonly effects: ReadonlyArray<DeliveryEffect>
}

const own = (state: DeliveryState): Delivery =>
  // SAFETY: the brand is type-only; this module's constructors are the sole producers of a Delivery.
  state as Delivery

export const initialDelivery = own({ phase: "idle" })

const closed = own({ phase: "closed" })

const debouncing = (debounce: Debounce): Delivery =>
  own({ phase: "debouncing", debounce })

const sending = (
  run: number,
  action: BacklightAction,
  next: Waiting | null
): Delivery => own({ phase: "sending", run, action, next })

const debounceOf = (selection: BacklightSelection, id: number): Debounce => ({
  kind: "debounce",
  selection,
  id,
})

const queued = (action: BacklightAction): Waiting => ({
  kind: "queued",
  action,
})

const cancel = (id: number): DeliveryEffect => ({ kind: "cancel", id })

const schedule = (id: number): DeliveryEffect => ({ kind: "schedule", id })

const send = (action: BacklightAction, run: number): DeliveryEffect => ({
  kind: "send",
  action,
  run,
})

const cancelWaiting = (waiting: Waiting | null): DeliveryEffect[] =>
  waiting?.kind === "debounce" ? [cancel(waiting.id)] : []

const step = (
  delivery: Delivery,
  effects: ReadonlyArray<DeliveryEffect> = []
): DeliveryStep => ({ delivery, effects })

function edit(
  delivery: Delivery,
  selection: BacklightSelection,
  id: number
): DeliveryStep {
  const debounce = debounceOf(selection, id)

  switch (delivery.phase) {
    case "closed":
      return step(delivery)
    case "sending":
      return step(sending(delivery.run, delivery.action, debounce), [
        ...cancelWaiting(delivery.next),
        schedule(id),
      ])
    case "debouncing":
      return step(debouncing(debounce), [
        cancel(delivery.debounce.id),
        schedule(id),
      ])
    case "idle":
      return step(debouncing(debounce), [schedule(id)])
  }
}

function perform(
  delivery: Delivery,
  action: BacklightAction,
  run: number
): DeliveryStep {
  switch (delivery.phase) {
    case "closed":
      return step(delivery)
    case "sending":
      return step(
        sending(delivery.run, delivery.action, queued(action)),
        cancelWaiting(delivery.next)
      )
    case "debouncing":
      return step(sending(run, action, null), [
        cancel(delivery.debounce.id),
        send(action, run),
      ])
    case "idle":
      return step(sending(run, action, null), [send(action, run)])
  }
}

function fire(delivery: Delivery, id: number, run: number): DeliveryStep {
  if (delivery.phase === "debouncing" && delivery.debounce.id === id) {
    const action: BacklightAction = {
      action: "select",
      selection: delivery.debounce.selection,
    }

    return step(sending(run, action, null), [send(action, run)])
  }

  if (
    delivery.phase === "sending" &&
    delivery.next?.kind === "debounce" &&
    delivery.next.id === id
  ) {
    const action: BacklightAction = {
      action: "select",
      selection: delivery.next.selection,
    }

    return step(sending(delivery.run, delivery.action, queued(action)))
  }

  return step(delivery)
}

function settle(delivery: Delivery, run: number): DeliveryStep {
  if (delivery.phase !== "sending") return step(delivery)

  const waiting = delivery.next

  if (waiting === null) return step(initialDelivery)

  if (waiting.kind === "queued")
    return step(sending(run, waiting.action, null), [send(waiting.action, run)])

  return step(debouncing(waiting))
}

function close(delivery: Delivery): DeliveryStep {
  switch (delivery.phase) {
    case "debouncing":
      return step(closed, [cancel(delivery.debounce.id)])
    case "sending":
      return step(closed, cancelWaiting(delivery.next))
    case "idle":
    case "closed":
      return step(closed)
  }
}

export function next(delivery: Delivery, event: DeliveryEvent): DeliveryStep {
  switch (event.type) {
    case "edit":
      return edit(delivery, event.selection, event.id)
    case "perform":
      return perform(delivery, event.action, event.run)
    case "fire":
      return fire(delivery, event.id, event.run)
    case "settle":
      return settle(delivery, event.run)
    case "close":
      return close(delivery)
  }
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
