// @vitest-environment happy-dom
import { act } from "react"
import {
  createMemoryHistory,
  createRootRoute,
  createRouter,
  RouterContextProvider,
} from "@tanstack/react-router"
import { createRoot, type Root } from "react-dom/client"
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest"
import { BinderBook } from "@/components/binder-book"
import type { BinderEntry, BinderSearch } from "@/lib/binder"
import {
  grip,
  idle,
  nextTurn,
  outcomeOf,
  restAfter,
  settleDuration,
  tapSlop,
  type Turn,
  type TurnEvent,
} from "@/lib/binder-turn"
import { button, settle, sheet } from "./support/lists-dom"

const alex = { id: "5a02863d-0c3f-4e11-9a2e-2e99334b5a93", name: "Alex" }

const entries: BinderEntry[] = Array.from({ length: 20 }, (_, slot) => {
  const code = `OP99-${String(slot + 1).padStart(3, "0")}`

  return {
    id: `00000000-0000-4000-8000-${String(slot + 1).padStart(12, "0")}`,
    owner_id: alex.id,
    language: "en",
    quantity: 1,
    note: null,
    date_added: "2026-11-01",
    card: {
      id: code,
      card_code: code,
      name: `Invented Card ${slot + 1}`,
      set_id: "OP99",
      set_name: "Synthetic Tides",
      rarity: null,
      color: null,
      card_type: null,
      life: null,
      cost: null,
      power: null,
      counter_amount: null,
      attribute: null,
      sub_types: null,
      text: null,
    },
  }
})

const box = { left: 0, right: 1000, top: 0, bottom: 600 }

let root: Root

let container: HTMLDivElement

let search: BinderSearch = {}

let reduced = false

beforeEach(() => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  reduced = false
  vi.stubGlobal("matchMedia", (query: string) => ({
    matches: query.includes("reduce") && reduced,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
  }))
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
    ...box,
    x: box.left,
    y: box.top,
    width: box.right - box.left,
    height: box.bottom - box.top,
    toJSON: () => box,
  })
  search = { person: alex.id }
  container = document.createElement("div")
  document.body.append(container)
  root = createRoot(container)
})

afterEach(async () => {
  await act(async () => root.unmount())
  container.remove()
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

async function render(): Promise<void> {
  const router = createRouter({
    routeTree: createRootRoute(),
    history: createMemoryHistory(),
    isServer: false,
  })

  await act(async () =>
    root.render(
      <RouterContextProvider router={router}>
        <BinderBook
          data={{ state: "available", owners: [alex], entries }}
          search={search}
          choose={(next) => {
            search = next
            void render()
          }}
          change={async () => ({ state: "saved" })}
          refresh={async () => undefined}
        />
      </RouterContextProvider>
    )
  )
}

const sleep = (ms: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, ms))

const folio = (): string =>
  container.querySelector(".binder-folio")?.textContent ?? ""

const shownPages = (): string[] =>
  Array.from(
    container.querySelectorAll(".binder-page"),
    (page) => page.getAttribute("aria-label") ?? ""
  ).sort()

const pocket = (number: number): HTMLButtonElement =>
  button(new RegExp(`^Invented Card ${number},`), container)

const spread = (): HTMLElement => {
  const element = container.querySelector<HTMLElement>(".binder-spread")

  if (element === null) throw new Error("Expected a spread")

  return element
}

function dispatchPointer(
  target: Element,
  type: string,
  x: number,
  y = 300
): void {
  target.dispatchEvent(
    new PointerEvent(type, {
      bubbles: true,
      cancelable: true,
      pointerId: 7,
      pointerType: "touch",
      isPrimary: true,
      button: 0,
      clientX: x,
      clientY: y,
    })
  )
}

function pointer(target: Element, type: string, x: number): Promise<void> {
  return act(async () => dispatchPointer(target, type, x))
}

async function slide(target: Element, from: number, to: number): Promise<void> {
  await pointer(target, "pointerdown", from)

  for (let step = 1; step <= 10; step++)
    await pointer(target, "pointermove", from + ((to - from) * step) / 10)
}

async function holdAndLift(target: Element, at: number): Promise<void> {
  await act(() => sleep(restAfter + 50))
  await pointer(target, "pointerup", at)
}

// A browser follows pointerup on a button with click in the same task; dispatched pointer events do not, so the test clicks.
async function lift(target: HTMLElement, at: number): Promise<void> {
  await act(async () => {
    dispatchPointer(target, "pointerup", at)
    target.click()
  })
  await settle()
}

function key(name: string): Promise<void> {
  return act(async () => {
    window.dispatchEvent(new KeyboardEvent("keydown", { key: name }))
  })
}

async function tap(target: Element, at: number): Promise<void> {
  await pointer(target, "pointerdown", at)
  await pointer(target, "pointerup", at)
}

async function landed(): Promise<void> {
  await act(() => sleep(settleDuration + 100))
  await settle()
}

describe("dragging a page", () => {
  test("from the right edge past the midpoint turns to the next spread", async () => {
    await render()

    await slide(spread(), 980, 400)
    expect(shownPages()).toEqual(["Page 1", "Page 2", "Page 3", "Page 4"])
    expect(folio()).toBe("Pages 1–2 of 4")

    await holdAndLift(spread(), 400)
    await landed()
    expect(folio()).toBe("Pages 3–4 of 4")
    expect(shownPages()).toEqual(["Page 3", "Page 4"])
    expect(search).toEqual({ person: alex.id, page: 2 })
    expect(pocket(19)).toBeDefined()
  })

  test("from the left edge past the midpoint turns back", async () => {
    search = { person: alex.id, page: 2 }
    await render()

    await slide(spread(), 20, 600)
    await holdAndLift(spread(), 600)
    await landed()
    expect(folio()).toBe("Pages 1–2 of 4")
    expect(shownPages()).toEqual(["Page 1", "Page 2"])
  })

  test("released short of the midpoint springs back to the same spread", async () => {
    await render()

    await slide(spread(), 980, 700)
    expect(shownPages()).toContain("Page 4")

    await holdAndLift(spread(), 700)
    await landed()
    expect(folio()).toBe("Pages 1–2 of 4")
    expect(shownPages()).toEqual(["Page 1", "Page 2"])
    expect(search).toEqual({ person: alex.id })
  })

  test("a cancelled pointer springs back even past the midpoint", async () => {
    await render()

    await slide(spread(), 980, 200)
    await pointer(spread(), "pointercancel", 200)
    await landed()
    expect(folio()).toBe("Pages 1–2 of 4")
    expect(shownPages()).toEqual(["Page 1", "Page 2"])
  })

  test("never starts before the first spread or away from the edges", async () => {
    await render()

    await slide(spread(), 20, 900)
    await holdAndLift(spread(), 900)
    await slide(spread(), 500, 0)
    await holdAndLift(spread(), 0)
    await landed()
    expect(folio()).toBe("Pages 1–2 of 4")
    expect(shownPages()).toEqual(["Page 1", "Page 2"])
  })
})

describe("tapping a page's edge", () => {
  test("on the right turns forward and on the left turns back", async () => {
    await render()

    await tap(spread(), 980)
    expect(shownPages()).toEqual(["Page 1", "Page 2", "Page 3", "Page 4"])
    expect(folio()).toBe("Pages 1–2 of 4")
    await landed()
    expect(folio()).toBe("Pages 3–4 of 4")

    await tap(spread(), 20)
    await landed()
    expect(folio()).toBe("Pages 1–2 of 4")
    expect(shownPages()).toEqual(["Page 1", "Page 2"])
  })

  test("turns nothing before the first spread, after the last or away from the edges", async () => {
    await render()

    await tap(spread(), 20)
    await tap(spread(), 500)
    await landed()
    expect(folio()).toBe("Pages 1–2 of 4")
    expect(shownPages()).toEqual(["Page 1", "Page 2"])

    search = { person: alex.id, page: 2 }
    await render()
    await tap(spread(), 980)
    await landed()
    expect(folio()).toBe("Pages 3–4 of 4")
    expect(shownPages()).toEqual(["Page 3", "Page 4"])
  })

  test("a press that leaves the spread ends, and the arrows still turn", async () => {
    await render()

    await pointer(spread(), "pointerdown", 995)
    await pointer(spread(), "pointermove", 1003)
    await pointer(spread(), "pointerout", 1003)
    await act(async () => button("Next pages").click())
    await landed()
    expect(folio()).toBe("Pages 3–4 of 4")
  })
})

describe("taps and drags on pockets", () => {
  test("a tap on a pocket at the edge opens its sheet and turns nothing", async () => {
    await render()
    const card = pocket(12)

    await pointer(card, "pointerdown", 950)
    await pointer(card, "pointermove", 953)
    await lift(card, 953)
    await landed()

    expect(sheet().textContent).toContain("Invented Card 12")
    expect(folio()).toBe("Pages 1–2 of 4")
  })

  test("a drag refused before the first spread opens nothing", async () => {
    await render()
    const card = pocket(1)

    await slide(card, 20, 300)
    await lift(card, 300)
    await landed()

    expect(document.querySelector(".sheet")).toBeNull()
    expect(folio()).toBe("Pages 1–2 of 4")
  })

  test("a pocket opened from the keyboard after a drag springs back opens its sheet", async () => {
    await render()

    await slide(spread(), 980, 700)
    await holdAndLift(spread(), 700)
    await landed()
    await act(async () => pocket(12).click())
    await settle()

    expect(sheet().textContent).toContain("Invented Card 12")
  })

  test("a drag that starts on a pocket turns the page and never opens the sheet", async () => {
    await render()
    const card = pocket(12)

    await slide(card, 950, 300)
    await act(() => sleep(restAfter + 50))
    await lift(card, 300)
    await landed()

    expect(document.querySelector(".sheet")).toBeNull()
    expect(folio()).toBe("Pages 3–4 of 4")
  })

  test("a drag on a pocket that springs back opens nothing, and the next tap does", async () => {
    await render()
    const card = pocket(12)

    await slide(card, 950, 800)
    await act(() => sleep(restAfter + 50))
    await lift(card, 800)
    await landed()

    expect(document.querySelector(".sheet")).toBeNull()
    expect(folio()).toBe("Pages 1–2 of 4")

    await pointer(card, "pointerdown", 950)
    await lift(card, 950)
    expect(sheet().textContent).toContain("Invented Card 12")
  })

  test("a pocket opens its sheet after a turn", async () => {
    await render()

    await slide(spread(), 980, 300)
    await holdAndLift(spread(), 300)
    await landed()

    const card = pocket(20)
    await pointer(card, "pointerdown", 400)
    await lift(card, 400)

    expect(sheet().textContent).toContain("Invented Card 20")
  })
})

describe("controls", () => {
  test("the arrows and the arrow keys turn the page", async () => {
    await render()

    await act(async () => button("Next pages").click())
    expect(shownPages()).toEqual(["Page 1", "Page 2", "Page 3", "Page 4"])
    expect(folio()).toBe("Pages 1–2 of 4")
    await landed()
    expect(folio()).toBe("Pages 3–4 of 4")

    await act(async () => button("Previous pages").click())
    await landed()
    expect(folio()).toBe("Pages 1–2 of 4")

    await key("ArrowRight")
    expect(shownPages()).toEqual(["Page 1", "Page 2", "Page 3", "Page 4"])
    expect(folio()).toBe("Pages 1–2 of 4")
    await landed()
    expect(folio()).toBe("Pages 3–4 of 4")

    await key("ArrowRight")
    await landed()
    expect(folio()).toBe("Pages 3–4 of 4")

    await key("ArrowLeft")
    await landed()
    expect(folio()).toBe("Pages 1–2 of 4")
  })
})

describe("with reduced motion", () => {
  test("a drag swaps pages without a turning page", async () => {
    reduced = true
    await render()

    await slide(spread(), 980, 300)
    expect(shownPages()).toEqual(["Page 1", "Page 2"])

    await holdAndLift(spread(), 300)
    await settle()
    expect(folio()).toBe("Pages 3–4 of 4")
    expect(shownPages()).toEqual(["Page 3", "Page 4"])
  })

  test("the arrows, the arrow keys and an edge tap swap pages at once", async () => {
    reduced = true
    await render()

    await act(async () => button("Next pages").click())
    await settle()
    expect(folio()).toBe("Pages 3–4 of 4")
    expect(shownPages()).toEqual(["Page 3", "Page 4"])

    await key("ArrowLeft")
    await settle()
    expect(folio()).toBe("Pages 1–2 of 4")

    await tap(spread(), 980)
    await settle()
    expect(folio()).toBe("Pages 3–4 of 4")
    expect(shownPages()).toEqual(["Page 3", "Page 4"])
  })
})

describe("the turn decision", () => {
  test("past the midpoint completes and short of it cancels", () => {
    expect(outcomeOf(0.5, 0)).toBe("complete")
    expect(outcomeOf(0.49, 0)).toBe("cancel")
  })

  test("a flick decides whatever the progress", () => {
    expect(outcomeOf(0.1, 0.5)).toBe("complete")
    expect(outcomeOf(0.1, 0.49)).toBe("cancel")
    expect(outcomeOf(0.9, -0.5)).toBe("cancel")
    expect(outcomeOf(0.9, -0.49)).toBe("complete")
  })

  test("only the outer pocket column grips, and only toward a spread that exists", () => {
    const both = { back: true, forward: true }

    expect(grip(box, 834, both)).toEqual({ direction: "forward", reach: 834 })
    expect(grip(box, 832, both)).toBeUndefined()
    expect(grip(box, 166, both)).toEqual({ direction: "back", reach: 834 })
    expect(grip(box, 168, both)).toBeUndefined()
    expect(grip(box, 990, { back: true, forward: false })).toBeUndefined()
    expect(grip(box, 10, { back: false, forward: true })).toBeUndefined()
  })
})

describe("the turn lifecycle", () => {
  const run = (...events: TurnEvent[]): Turn => events.reduce(nextTurn, idle)

  const press: TurnEvent = {
    type: "press",
    pointer: 1,
    x: 900,
    y: 300,
    time: 0,
    pocket: false,
    grip: { direction: "forward", reach: 900 },
  }

  const release = (x: number, time: number): TurnEvent => ({
    type: "release",
    pointer: 1,
    x,
    time,
  })

  const move = (x: number, time: number, pointer = 1): TurnEvent => ({
    type: "move",
    pointer,
    x,
    y: 300,
    time,
  })

  test("a tap within the slop in a grip flips, on a pocket or away from a grip it ends idle", () => {
    const pressed = run(press, move(900 - tapSlop + 1, 10))

    expect(pressed.phase).toBe("pressing")
    expect(nextTurn(pressed, release(900, 20))).toEqual({
      phase: "settling",
      direction: "forward",
      outcome: "complete",
    })
    expect(run({ ...press, pocket: true }, release(900, 20))).toBe(idle)
    expect(run({ ...press, grip: undefined }, release(900, 20))).toBe(idle)
  })

  test("moving past the slop away from a grip is refused until release", () => {
    const refused = run({ ...press, grip: undefined }, move(900 - tapSlop, 10))

    expect(refused).toEqual({ phase: "refused", pointer: 1 })
    expect(nextTurn(refused, { type: "flip", direction: "forward" })).toBe(
      refused
    )
    expect(nextTurn(refused, release(800, 20))).toBe(idle)
    expect(nextTurn(refused, { type: "lose", pointer: 1 })).toBe(idle)
  })

  test("a press that loses its pointer ends idle", () => {
    expect(run(press, { type: "lose", pointer: 1 })).toBe(idle)
  })

  test("moving past the slop drags, following the pointer", () => {
    const dragging = run(press, move(900 - tapSlop, 10), move(450, 20))

    expect(dragging).toMatchObject({ phase: "dragging", progress: 0.5 })
    expect(run(press, move(900 - tapSlop, 10), move(1200, 20))).toMatchObject({
      progress: 0,
    })
    expect(run(press, move(800, 10), move(-500, 20))).toMatchObject({
      progress: 1,
    })
  })

  test("another pointer neither drags nor releases the turn", () => {
    const dragging = run(press, move(800, 10))

    expect(nextTurn(dragging, move(100, 20, 2))).toBe(dragging)
    expect(nextTurn(dragging, { type: "lose", pointer: 2 })).toBe(dragging)
  })

  test("a fast release completes, a rested one is decided by the midpoint", () => {
    const flicking = run(press, move(880, 10), move(800, 20))

    expect(nextTurn(flicking, release(800, 30))).toMatchObject({
      phase: "settling",
      outcome: "complete",
    })
    expect(nextTurn(flicking, release(800, 20 + restAfter + 1))).toMatchObject({
      phase: "settling",
      outcome: "cancel",
    })
  })

  test("a flick delivered as one move completes short of the midpoint", () => {
    expect(run(press, move(800, 10), release(800, 11))).toMatchObject({
      phase: "settling",
      outcome: "complete",
    })
  })

  test("a lost pointer springs back, and settling ends turned or idle", () => {
    const lost = run(press, move(100, 10), { type: "lose", pointer: 1 })

    expect(lost).toEqual({
      phase: "settling",
      direction: "forward",
      outcome: "cancel",
    })
    expect(nextTurn(lost, { type: "settled" })).toBe(idle)
    expect(
      run({ type: "flip", direction: "back" }, { type: "settled" })
    ).toEqual({ phase: "turned", direction: "back" })
  })

  test("a settling or turned page ignores new presses", () => {
    const settling = run({ type: "flip", direction: "forward" })

    expect(nextTurn(settling, press)).toBe(settling)

    const turned = nextTurn(settling, { type: "settled" })

    expect(nextTurn(turned, press)).toBe(turned)
  })
})
