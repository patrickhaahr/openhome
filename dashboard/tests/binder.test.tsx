// @vitest-environment happy-dom
// @vitest-environment-options {"settings":{"fetch":{"disableSameOriginPolicy":true}}}
// The private server transport calls Axum; browser CORS applies only to Start RPCs.
import { createServer } from "node:http"
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
import { z } from "zod"
import { BinderBook } from "@/components/binder-book"
import { paginate, type BinderSearch } from "@/lib/binder"
import {
  addBinderEntry,
  readBinder,
  readCardImage,
  searchBinderCards,
} from "@/lib/binder.server"
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
          find={searchBinderCards}
          change={addBinderEntry}
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

const pageCards = (number: number): (string | undefined)[] =>
  Array.from(
    container.querySelectorAll<HTMLElement>(
      `[aria-label="Page ${number}"] .pocket:not([data-empty]) img`
    ),
    (image) => image.getAttribute("src")?.replace("/binder-image/", "")
  )

const cards = (): (string | undefined)[] =>
  pockets().map((pocket) =>
    pocket
      .querySelector("img")
      ?.getAttribute("src")
      ?.replace("/binder-image/", "")
  )

/** Twenty entries, added out of Card Code order: the first spread is full and the second holds two. */
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
  ...["OP99-001", "OP99-003", "OP99-005", "OP99-006", "OP99-010"].map(
    (card): Addition => ({ card, language: "ja" })
  ),
  { card: "OP99-011", language: "ja" },
  { card: "OP99-020", language: "ja" },
]

const firstSpread = [
  "OP99-001",
  "OP99-001",
  "OP99-001_p1",
  "OP99-002",
  "OP99-002_p1",
  "OP99-003",
  "OP99-003",
  "OP99-004",
  "OP99-004",
  "OP99-004_p1",
  "OP99-005",
  "OP99-005",
  "OP99-006",
  "OP99-006",
  "OP99-010",
  "OP99-010",
  "OP99-011",
  "OP99-011",
]

test("lays out a spread of two facing nine-pocket pages in Card Code order, marking copies and Japanese entries", async () => {
  await add(await owner("Alex"), ...alex)
  await render()

  expect(container.querySelectorAll(".binder-page")).toHaveLength(2)
  expect(container.querySelectorAll(".pocket")).toHaveLength(18)
  expect(pageCards(1)).toEqual(firstSpread.slice(0, 9))
  expect(pageCards(2)).toEqual(firstSpread.slice(9))
  expect(pageCards(2)[0]).toBe("OP99-004_p1")
  expect(container.textContent).toContain("Pages 1–2 of 4")
  expect(button("Previous pages").disabled).toBe(true)

  expect(
    pockets().map(
      (pocket) => pocket.querySelector(".pocket-quantity")?.textContent ?? null
    )
  ).toEqual(firstSpread.map((_, slot) => (slot === 3 ? "×3" : null)))
  expect(
    pockets().map(
      (pocket) => pocket.querySelector(".pocket-mark")?.textContent ?? null
    )
  ).toEqual(
    firstSpread.map((_, slot) =>
      [1, 6, 8, 11, 13, 15, 17].includes(slot) ? "JP" : null
    )
  )

  for (const image of container.querySelectorAll("img"))
    expect(image.getAttribute("src")).toMatch(/^\/binder-image\//)

  expect(container.textContent).toContain("OPTCG catalogue")
})

test("turns a whole spread by the arrows and the arrow keys, leaving unfilled pockets empty", async () => {
  await add(await owner("Alex"), ...alex)
  await render()

  await press(button("Next pages"))
  expect(pageCards(3)).toEqual(["OP99-020", "OP99-020"])
  expect(pageCards(4)).toEqual([])
  expect(container.querySelectorAll(".pocket")).toHaveLength(18)
  expect(container.querySelectorAll(".pocket[data-empty]")).toHaveLength(16)
  expect(container.textContent).toContain("Pages 3–4 of 4")
  expect(button("Next pages").disabled).toBe(true)

  await act(async () => {
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowLeft" }))
    await refreshWork
  })
  await settle()
  expect(container.textContent).toContain("Pages 1–2 of 4")
  expect(cards()).toEqual(firstSpread)

  await act(async () => {
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight" }))
    await refreshWork
  })
  await settle()
  expect(container.textContent).toContain("Pages 3–4 of 4")
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
  expect(container.textContent).not.toContain("Pages 1")
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

test("relays no stored card image larger than 2 MiB", async () => {
  const oversize = createServer((_request, response) => {
    response.writeHead(200, { "content-type": "image/avif" })
    response.end(Buffer.alloc(2 * 1024 * 1024 + 1))
  })

  await new Promise<void>((done) => oversize.listen(0, "127.0.0.1", done))

  const { port } = z.object({ port: z.number() }).parse(oversize.address())

  vi.stubEnv("OPENHOME_API_URL", `http://127.0.0.1:${port}`)

  try {
    expect((await readCardImage({ card: "OP99-001" })).status).toBe(503)
  } finally {
    await new Promise((done) => oversize.close(done))
  }
})

test("a spread is two nine-pocket pages, with the index kept in range", () => {
  const entries = Array.from({ length: 40 }, (_, index) => index)

  const first = paginate(entries, 0)

  expect(first.left).toEqual(entries.slice(0, 9))
  expect(first.right).toEqual(entries.slice(9, 18))
  expect(paginate(entries, 2)).toMatchObject({
    left: [36, 37, 38, 39],
    right: [],
    index: 2,
    count: 3,
  })
  expect(paginate(entries, 7).index).toBe(2)
  expect(paginate(entries, -1).index).toBe(0)
  expect(paginate(entries.slice(0, 18), 1)).toMatchObject({
    index: 0,
    count: 1,
  })
  expect(paginate([], 0)).toEqual({ left: [], right: [], index: 0, count: 1 })
})
