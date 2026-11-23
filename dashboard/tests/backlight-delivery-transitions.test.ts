import { describe, expect, expectTypeOf, test } from "vitest"
import {
  initialDelivery,
  next,
  type Delivery,
  type DeliveryEvent,
} from "@/components/backlight-delivery"
import type { BacklightAction, BacklightSelection } from "@/lib/backlight"

const idA = 1

const idB = 2

const off: BacklightSelection = { mode: "off" }

const tvSync: BacklightSelection = { mode: "tv-sync" }

const selectOff: BacklightAction = { action: "select", selection: off }

const selectTvSync: BacklightAction = { action: "select", selection: tvSync }

const refresh: BacklightAction = { action: "refresh" }

function run(events: ReadonlyArray<DeliveryEvent>): Delivery {
  return events.reduce(
    (delivery, event) => next(delivery, event).delivery,
    initialDelivery
  )
}

describe("edit", () => {
  test("starts debouncing from idle", () => {
    const step = next(initialDelivery, {
      type: "edit",
      selection: off,
      id: idA,
    })

    expect(step.delivery).toEqual({
      phase: "debouncing",
      debounce: { kind: "debounce", selection: off, id: idA },
    })
    expect(step.effects).toEqual([{ kind: "schedule", id: idA }])
  })

  test("replaces a pending debounce, cancelling the old timer before scheduling the new", () => {
    const step = next(run([{ type: "edit", selection: off, id: idA }]), {
      type: "edit",
      selection: tvSync,
      id: idB,
    })

    expect(step.delivery).toEqual({
      phase: "debouncing",
      debounce: { kind: "debounce", selection: tvSync, id: idB },
    })
    expect(step.effects).toEqual([
      { kind: "cancel", id: idA },
      { kind: "schedule", id: idB },
    ])
  })

  test("waits behind the send in flight and replaces an earlier waiting debounce", () => {
    const sending = run([
      { type: "perform", action: refresh, run: 1 },
      { type: "edit", selection: off, id: idA },
    ])

    const step = next(sending, {
      type: "edit",
      selection: tvSync,
      id: idB,
    })

    expect(step.delivery).toEqual({
      phase: "sending",
      run: 1,
      action: refresh,
      next: { kind: "debounce", selection: tvSync, id: idB },
    })
    expect(step.effects).toEqual([
      { kind: "cancel", id: idA },
      { kind: "schedule", id: idB },
    ])
  })

  test("schedules nothing once closed", () => {
    const step = next(run([{ type: "close" }]), {
      type: "edit",
      selection: off,
      id: idA,
    })

    expect(step.delivery).toEqual({ phase: "closed" })
    expect(step.effects).toEqual([])
  })
})

describe("perform", () => {
  test("sends immediately from idle", () => {
    const step = next(initialDelivery, {
      type: "perform",
      action: refresh,
      run: 1,
    })

    expect(step.delivery).toEqual({
      phase: "sending",
      run: 1,
      action: refresh,
      next: null,
    })
    expect(step.effects).toEqual([{ kind: "send", action: refresh, run: 1 }])
  })

  test("cancels a pending debounce and sends", () => {
    const step = next(run([{ type: "edit", selection: off, id: idA }]), {
      type: "perform",
      action: refresh,
      run: 2,
    })

    expect(step.delivery).toMatchObject({ phase: "sending", run: 2 })
    expect(step.effects).toEqual([
      { kind: "cancel", id: idA },
      { kind: "send", action: refresh, run: 2 },
    ])
  })

  test("queues behind the send in flight, replacing the waiting debounce", () => {
    const sending = run([
      { type: "perform", action: refresh, run: 1 },
      { type: "edit", selection: off, id: idA },
    ])

    const step = next(sending, {
      type: "perform",
      action: selectTvSync,
      run: 2,
    })

    expect(step.delivery).toEqual({
      phase: "sending",
      run: 1,
      action: refresh,
      next: { kind: "queued", action: selectTvSync },
    })
    expect(step.effects).toEqual([{ kind: "cancel", id: idA }])
  })

  test("is ignored once closed", () => {
    const step = next(run([{ type: "close" }]), {
      type: "perform",
      action: refresh,
      run: 1,
    })

    expect(step).toEqual({ delivery: { phase: "closed" }, effects: [] })
  })
})

describe("fire", () => {
  test("sends the debounced selection", () => {
    const step = next(run([{ type: "edit", selection: off, id: idA }]), {
      type: "fire",
      id: idA,
      run: 1,
    })

    expect(step.delivery).toEqual({
      phase: "sending",
      run: 1,
      action: selectOff,
      next: null,
    })
    expect(step.effects).toEqual([{ kind: "send", action: selectOff, run: 1 }])
  })

  test("queues the debounced selection behind the send in flight", () => {
    const sending = run([
      { type: "perform", action: refresh, run: 1 },
      { type: "edit", selection: off, id: idA },
    ])

    const step = next(sending, { type: "fire", id: idA, run: 2 })

    expect(step.delivery).toEqual({
      phase: "sending",
      run: 1,
      action: refresh,
      next: { kind: "queued", action: selectOff },
    })
    expect(step.effects).toEqual([])
  })

  test("ignores a timer that is no longer current", () => {
    const debouncing = run([{ type: "edit", selection: off, id: idB }])

    expect(next(debouncing, { type: "fire", id: idA, run: 1 })).toEqual({
      delivery: debouncing,
      effects: [],
    })
  })
})

describe("settle", () => {
  test("returns to idle when nothing waits", () => {
    const step = next(run([{ type: "perform", action: refresh, run: 1 }]), {
      type: "settle",
      run: 2,
    })

    expect(step).toEqual({ delivery: initialDelivery, effects: [] })
  })

  test("sends the queued action under the new run", () => {
    const sending = run([
      { type: "perform", action: refresh, run: 1 },
      { type: "perform", action: selectTvSync, run: 2 },
    ])

    const step = next(sending, { type: "settle", run: 3 })

    expect(step.delivery).toEqual({
      phase: "sending",
      run: 3,
      action: selectTvSync,
      next: null,
    })
    expect(step.effects).toEqual([
      { kind: "send", action: selectTvSync, run: 3 },
    ])
  })

  test("resumes debouncing when a debounce was waiting", () => {
    const sending = run([
      { type: "perform", action: refresh, run: 1 },
      { type: "edit", selection: off, id: idA },
    ])

    const step = next(sending, { type: "settle", run: 2 })

    expect(step.delivery).toEqual({
      phase: "debouncing",
      debounce: { kind: "debounce", selection: off, id: idA },
    })
    expect(step.effects).toEqual([])
  })

  test("is ignored outside a send", () => {
    expect(next(initialDelivery, { type: "settle", run: 1 })).toEqual({
      delivery: initialDelivery,
      effects: [],
    })
  })
})

describe("close", () => {
  test("cancels a pending debounce", () => {
    const step = next(run([{ type: "edit", selection: off, id: idA }]), {
      type: "close",
    })

    expect(step).toEqual({
      delivery: { phase: "closed" },
      effects: [{ kind: "cancel", id: idA }],
    })
  })

  test("cancels the debounce waiting behind a send", () => {
    const sending = run([
      { type: "perform", action: refresh, run: 1 },
      { type: "edit", selection: off, id: idA },
    ])

    expect(next(sending, { type: "close" })).toEqual({
      delivery: { phase: "closed" },
      effects: [{ kind: "cancel", id: idA }],
    })
  })

  test("closes an idle delivery without effects", () => {
    expect(next(initialDelivery, { type: "close" })).toEqual({
      delivery: { phase: "closed" },
      effects: [],
    })
  })
})

describe("Delivery", () => {
  test("cannot be built from a literal outside the module", () => {
    expectTypeOf<{ readonly phase: "idle" }>().not.toExtend<Delivery>()
    expectTypeOf<{
      readonly phase: "sending"
      readonly run: number
      readonly action: BacklightAction
      readonly next: null
    }>().not.toExtend<Delivery>()
  })
})
