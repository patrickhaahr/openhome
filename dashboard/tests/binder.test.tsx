// @vitest-environment happy-dom
// @vitest-environment-options {"settings":{"fetch":{"disableSameOriginPolicy":true}}}
// The private server transport calls Axum; browser CORS applies only to Start RPCs.
import { act } from "react"
import {
  createMemoryHistory,
  createRootRoute,
  createRouter,
  RouterContextProvider,
} from "@tanstack/react-router"
import { createRoot, type Root } from "react-dom/client"
import {
  afterAll,
  afterEach,
  beforeAll,
  beforeEach,
  expect,
  test,
  vi,
} from "vitest"
import { BinderBook } from "@/components/binder-book"
import { paginate, type BinderSearch } from "@/lib/binder"
import { readBinder, readCardImage } from "@/lib/binder.server"
import { readWatchlists } from "@/lib/watchlist.server"
import { writeWishlist } from "@/lib/wishlist.server"
import { startAxumFixture } from "./support/axum-fixture"
import { button, nameOf, settle } from "./support/lists-dom"

// The fixture's OPTCG stand-in knows the invented OP99 cards only.
let fixture: Awaited<ReturnType<typeof startAxumFixture>>

let root: Root

let container: HTMLDivElement

let search: BinderSearch = {}

let refreshWork: Promise<void> = Promise.resolve()

const key = "synthetic-dashboard-api-key"

beforeAll(async () => {
  fixture = await startAxumFixture()
})

afterAll(async () => {
  await fixture.stop()
})

beforeEach(() => {
  vi.stubEnv("OPENHOME_API_URL", fixture.url)
  vi.stubEnv("API_KEY", key)
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  search = {}
  refreshWork = Promise.resolve()
  container = document.createElement("div")
  document.body.append(container)
  root = createRoot(container)
})

afterEach(async () => {
  await act(async () => root.unmount())
  container.remove()
  vi.stubEnv("OPENHOME_API_URL", fixture.url)
  const data = await readWatchlists()

  if (data.state !== "available") throw new Error("Expected fixture discovery")

  for (const owner of data.owners)
    await writeWishlist({ kind: "remove-owner", owner_id: owner.id })
  vi.unstubAllEnvs()
  vi.unstubAllGlobals()
})

async function render(): Promise<void> {
  const data = await readBinder()

  const router = createRouter({
    routeTree: createRootRoute(),
    history: createMemoryHistory(),
    isServer: false,
  })

  await act(async () =>
    root.render(
      <RouterContextProvider router={router}>
        <BinderBook
          data={data}
          search={search}
          choose={(next) => {
            search = next
            refreshWork = render()
          }}
          refresh={() => {
            vi.stubEnv("OPENHOME_API_URL", fixture.url)
            refreshWork = render()

            return refreshWork
          }}
        />
      </RouterContextProvider>
    )
  )
}

async function press(element: HTMLButtonElement): Promise<void> {
  await act(async () => {
    element.click()
    await refreshWork
  })
  await settle()
}

async function owner(name: string): Promise<string> {
  await writeWishlist({ kind: "create-owner", name })
  const data = await readBinder()

  const found =
    data.state === "available"
      ? data.owners.find((candidate) => candidate.name === name)
      : undefined

  if (found === undefined) throw new Error(`Missing owner ${name}`)

  return found.id
}

type Addition = { card: string; quantity?: number; language?: "en" | "ja" }

async function add(ownerId: string, ...additions: Addition[]): Promise<void> {
  for (const [index, addition] of additions.entries()) {
    const response = await fetch(`${fixture.url}/api/binder/entries`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${key}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        owner_id: ownerId,
        card_id: addition.card,
        quantity: addition.quantity,
        language: addition.language,
        request_id: `${ownerId}-${index}`,
      }),
    })

    if (!response.ok)
      throw new Error(`Binder addition failed ${response.status}`)
  }
}

const pockets = (): HTMLElement[] =>
  Array.from(container.querySelectorAll<HTMLElement>(".pocket")).filter(
    (pocket) => !pocket.hasAttribute("data-empty")
  )

const cards = (): (string | undefined)[] =>
  pockets().map((pocket) =>
    pocket
      .querySelector("img")
      ?.getAttribute("src")
      ?.replace("/binder-image/", "")
  )

/** Thirteen entries, added out of Card Code order. */
const alex: Addition[] = [
  { card: "OP99-010" },
  { card: "OP99-004_p1" },
  { card: "OP99-002", quantity: 3 },
  { card: "OP99-011" },
  { card: "OP99-001_p1" },
  { card: "OP99-004" },
  { card: "OP99-004", language: "ja" },
  { card: "OP99-006" },
  { card: "OP99-001" },
  { card: "OP99-005" },
  { card: "OP99-003" },
  { card: "OP99-002_p1" },
  { card: "OP99-020" },
]

test("lays out nine pockets a page in Card Code order, marking copies and Japanese entries, and pages by the arrows", async () => {
  await add(await owner("Alex"), ...alex)
  await render()

  expect(cards()).toEqual([
    "OP99-001",
    "OP99-001_p1",
    "OP99-002",
    "OP99-002_p1",
    "OP99-003",
    "OP99-004",
    "OP99-004",
    "OP99-004_p1",
    "OP99-005",
  ])
  expect(container.querySelectorAll(".pocket")).toHaveLength(9)
  expect(container.textContent).toContain("Page 1 of 2")
  expect(button("Previous page").disabled).toBe(true)

  expect(
    pockets().map(
      (pocket) => pocket.querySelector(".pocket-quantity")?.textContent ?? null
    )
  ).toEqual([null, null, "×3", null, null, null, null, null, null])
  expect(
    pockets().map(
      (pocket) => pocket.querySelector(".pocket-mark")?.textContent ?? null
    )
  ).toEqual([null, null, null, null, null, null, "JP", null, null])

  for (const image of container.querySelectorAll("img"))
    expect(image.getAttribute("src")).toMatch(/^\/binder-image\//)

  await press(button("Next page"))
  expect(cards()).toEqual(["OP99-006", "OP99-010", "OP99-011", "OP99-020"])
  expect(container.querySelectorAll(".pocket")).toHaveLength(9)
  expect(container.textContent).toContain("Page 2 of 2")
  expect(button("Next page").disabled).toBe(true)

  await act(async () => {
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowLeft" }))
    await refreshWork
  })
  await settle()
  expect(container.textContent).toContain("Page 1 of 2")
  expect(container.textContent).toContain("OPTCG catalogue")
})

test("switches between Household Users' Binders", async () => {
  const first = await owner("Alex")
  const second = await owner("Robin")
  await add(first, { card: "OP99-001" }, { card: "OP99-002" })
  await add(second, { card: "OP99-010", quantity: 4 })
  await render()

  const names = Array.from(container.querySelectorAll(".person-key"), nameOf)

  expect(names).toEqual(["Alex", "Robin"])
  expect(cards()).toEqual(["OP99-001", "OP99-002"])

  await press(button("Robin"))
  expect(cards()).toEqual(["OP99-010"])
  expect(pockets()[0]?.querySelector(".pocket-quantity")?.textContent).toBe(
    "×4"
  )
  expect(button("Robin").getAttribute("aria-pressed")).toBe("true")
})

test("says a Binder is empty rather than showing blank pages", async () => {
  await owner("Alex")
  await owner("Sam")
  await render()
  await press(button("Sam"))

  expect(container.textContent).toContain("Sam’s Binder is empty")
  expect(container.querySelectorAll(".pocket")).toHaveLength(0)
  expect(container.textContent).not.toContain("Page 1")
})

test("keeps recovery controls for a failed read and recovers when OpenHome answers", async () => {
  await add(await owner("Alex"), { card: "OP99-001" })
  vi.stubEnv("OPENHOME_API_URL", "http://127.0.0.1:9")
  await render()

  expect(container.textContent).toContain("Binder unavailable")
  expect(container.textContent).not.toContain("empty")
  expect(container.querySelectorAll(".pocket")).toHaveLength(0)

  await press(button("Check again"))
  expect(container.textContent).not.toContain("Binder unavailable")
  expect(cards()).toEqual(["OP99-001"])
})

test("relays a stored card image through Start and rejects anything else", async () => {
  await add(await owner("Alex"), { card: "OP99-001" })

  const image = await readCardImage({ card: "OP99-001" })

  expect(image.status).toBe(200)
  expect(image.headers.get("content-type")).toBe("image/avif")
  expect((await image.arrayBuffer()).byteLength).toBeGreaterThan(0)
  expect((await readCardImage({ card: "OP99-999" })).status).toBe(404)
  expect((await readCardImage({ card: "../entries" })).status).toBe(404)
})

test("pages are nine pockets, with the index kept in range", () => {
  const entries = Array.from({ length: 20 }, (_, index) => index)

  expect(paginate(entries, 0).pockets).toHaveLength(9)
  expect(paginate(entries, 2)).toMatchObject({ index: 2, count: 3 })
  expect(paginate(entries, 2).pockets).toHaveLength(2)
  expect(paginate(entries, 7).index).toBe(2)
  expect(paginate(entries, -1).index).toBe(0)
  expect(paginate([], 0)).toEqual({ pockets: [], index: 0, count: 1 })
})
