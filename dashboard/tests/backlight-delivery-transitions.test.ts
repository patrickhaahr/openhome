import { describe, expect, test } from "vitest"
import {
  initialDelivery,
  next,
  type Delivery,
  type DeliveryEvent,
} from "@/components/backlight-delivery"
import type { BacklightAction, BacklightSelection } from "@/lib/backlight"

const timerA = setTimeout(() => {}, 0)

const timerB = setTimeout(() => {}, 0)

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
      timer: timerA,
    })

    expect(step.delivery).toEqual({
      phase: "debouncing",
      debounce: { kind: "debounce", selection: off, timer: timerA },
    })
    expect(step.effects).toEqual([])
  })

  test("replaces a pending debounce and cancels its timer", () => {
    const step = next(run([{ type: "edit", selection: off, timer: timerA }]), {
      type: "edit",
      selection: tvSync,
      timer: timerB,
    })

    expect(step.delivery).toEqual({
      phase: "debouncing",
      debounce: { kind: "debounce", selection: tvSync, timer: timerB },
    })
    expect(step.effects).toEqual([{ kind: "cancel", timer: timerA }])
  })

  test("waits behind the send in flight and replaces an earlier waiting debounce", () => {
    const sending = run([
      { type: "perform", action: refresh, run: 1 },
      { type: "edit", selection: off, timer: timerA },
    ])

    const step = next(sending, {
      type: "edit",
      selection: tvSync,
      timer: timerB,
    })

    expect(step.delivery).toEqual({
      phase: "sending",
      run: 1,
      action: refresh,
      next: { kind: "debounce", selection: tvSync, timer: timerB },
    })
    expect(step.effects).toEqual([{ kind: "cancel", timer: timerA }])
  })

  test("cancels the new timer once closed", () => {
    const step = next(run([{ type: "close" }]), {
      type: "edit",
      selection: off,
      timer: timerA,
    })

    expect(step.delivery).toEqual({ phase: "closed" })
    expect(step.effects).toEqual([{ kind: "cancel", timer: timerA }])
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
    const step = next(run([{ type: "edit", selection: off, timer: timerA }]), {
      type: "perform",
      action: refresh,
      run: 2,
    })

    expect(step.delivery).toMatchObject({ phase: "sending", run: 2 })
    expect(step.effects).toEqual([
      { kind: "cancel", timer: timerA },
      { kind: "send", action: refresh, run: 2 },
    ])
  })

  test("queues behind the send in flight, replacing the waiting debounce", () => {
    const sending = run([
      { type: "perform", action: refresh, run: 1 },
      { type: "edit", selection: off, timer: timerA },
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
    expect(step.effects).toEqual([{ kind: "cancel", timer: timerA }])
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
    const step = next(run([{ type: "edit", selection: off, timer: timerA }]), {
      type: "fire",
      timer: timerA,
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
      { type: "edit", selection: off, timer: timerA },
    ])

    const step = next(sending, { type: "fire", timer: timerA, run: 2 })

    expect(step.delivery).toEqual({
      phase: "sending",
      run: 1,
      action: refresh,
      next: { kind: "queued", action: selectOff },
    })
    expect(step.effects).toEqual([])
  })

  test("ignores a timer that is no longer current", () => {
    const debouncing = run([{ type: "edit", selection: off, timer: timerB }])

    expect(next(debouncing, { type: "fire", timer: timerA, run: 1 })).toEqual({
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
      { type: "edit", selection: off, timer: timerA },
    ])

    const step = next(sending, { type: "settle", run: 2 })

    expect(step.delivery).toEqual({
      phase: "debouncing",
      debounce: { kind: "debounce", selection: off, timer: timerA },
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
    const step = next(run([{ type: "edit", selection: off, timer: timerA }]), {
      type: "close",
    })

    expect(step).toEqual({
      delivery: { phase: "closed" },
      effects: [{ kind: "cancel", timer: timerA }],
    })
  })

  test("cancels the debounce waiting behind a send", () => {
    const sending = run([
      { type: "perform", action: refresh, run: 1 },
      { type: "edit", selection: off, timer: timerA },
    ])

    expect(next(sending, { type: "close" })).toEqual({
      delivery: { phase: "closed" },
      effects: [{ kind: "cancel", timer: timerA }],
    })
  })

  test("closes an idle delivery without effects", () => {
    expect(next(initialDelivery, { type: "close" })).toEqual({
      delivery: { phase: "closed" },
      effects: [],
    })
  })
})
