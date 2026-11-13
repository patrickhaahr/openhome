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
import type { BinderMutation } from "@/lib/binder-edit"
import { settleDuration } from "@/lib/binder-turn"
import { readBinder, readCardImage, writeBinder } from "@/lib/binder.server"
import { readWatchlists } from "@/lib/watchlist.server"
import { writeWishlist } from "@/lib/wishlist.server"
import { startAxumFixture } from "./support/axum-fixture"
import { button, field, nameOf, settle, sheet, type } from "./support/lists-dom"

// The fixture's OPTCG stand-in knows the invented OP99 cards only.
let fixture: Awaited<ReturnType<typeof startAxumFixture>>

let root: Root

let container: HTMLDivElement

let search: BinderSearch = {}

let refreshWork: Promise<void> = Promise.resolve()

let work: Promise<BinderMutation> = Promise.resolve({ state: "saved" })

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
  work = Promise.resolve({ state: "saved" })
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
          change={(command) => {
            work = writeBinder(command)

            return work
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
    await work
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
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, settleDuration + 100))
    await refreshWork
  })
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

async function stored() {
  const data = await readBinder()

  if (data.state !== "available") throw new Error("Expected a Binder read")

  return data
}

/** The note field; its label cannot be matched by text once the textarea holds one. */
function note(): HTMLTextAreaElement {
  const found = sheet().querySelector("textarea")

  if (found === null) throw new Error("Missing note field")

  return found
}

async function openCard(name: RegExp): Promise<void> {
  await press(button(name, container))
}

test("a pocket opens a sheet with the large image and the card's details", async () => {
  const alex = await owner("Alex")
  await add(alex, { card: "OP99-001", quantity: 2 })
  await render()

  await openCard(/^Captain Marlow \(001\), OP99-001, 2 copies/)

  const open = sheet()

  expect(open.querySelector("img")?.getAttribute("src")).toBe(
    "/binder-image/OP99-001"
  )
  expect(open.querySelector(".sheet-title")?.textContent).toBe(
    "Captain Marlow (001)"
  )

  const facts = Array.from(open.querySelectorAll(".binder-facts div"), (row) =>
    nameOf(row)
  )

  expect(facts).toEqual([
    "SetSynthetic Tides",
    "RarityL",
    "ColourRed",
    "TypeLeader",
    "Power5000",
    "Life5",
    "AttributeSlash",
    "TypesLantern Crew Invented",
  ])
  expect(open.textContent).toContain("[DON!! x1] [Your Turn]")
  expect(open.textContent).toContain("OP99-001")
  expect(field("Quantity", open).value).toBe("2")
  expect(button("EN", open).getAttribute("aria-pressed")).toBe("true")
  expect(button("JP", open).getAttribute("aria-pressed")).toBe("false")
  expect(button("Save changes", open).disabled).toBe(true)
  expect(container.querySelector(".binder-page")).not.toBeNull()
  expect(container.querySelectorAll(".binder-page")).toHaveLength(2)
})

test("edits quantity, language and note and reads the stored entry back, clearing a note with an empty one", async () => {
  const alex = await owner("Alex")
  await add(alex, { card: "OP99-001" })
  await render()
  await openCard(/^Captain Marlow/)

  await type(field("Quantity", sheet()), "7")
  await type(note(), "  sleeved, binder 2 ")
  await press(button("Save changes", sheet()))

  expect((await stored()).entries[0]).toMatchObject({
    quantity: 7,
    language: "en",
    note: "sleeved, binder 2",
  })
  expect(field("Quantity", sheet()).value).toBe("7")
  expect(note().value).toBe("sleeved, binder 2")
  expect(button("Save changes", sheet()).disabled).toBe(true)
  expect(pockets()[0]?.querySelector(".pocket-quantity")?.textContent).toBe(
    "×7"
  )

  await press(button("JP", sheet()))
  await press(button("Save changes", sheet()))
  expect((await stored()).entries[0]).toMatchObject({
    quantity: 7,
    language: "ja",
    note: "sleeved, binder 2",
  })
  expect(button("JP", sheet()).getAttribute("aria-pressed")).toBe("true")
  expect(pockets()[0]?.querySelector(".pocket-mark")?.textContent).toBe("JP")

  await type(note(), "")
  await press(button("Save changes", sheet()))
  expect((await stored()).entries[0]).toMatchObject({
    quantity: 7,
    language: "ja",
    note: null,
  })
  expect(note().value).toBe("")

  await type(field("Quantity", sheet()), "0")
  expect(button("Save changes", sheet()).disabled).toBe(true)
  await type(field("Quantity", sheet()), "1.5")
  expect(button("Save changes", sheet()).disabled).toBe(true)
})

test("removes an entry only after confirmation, keeping the other entries and the card", async () => {
  const alex = await owner("Alex")
  await add(alex, { card: "OP99-001" }, { card: "OP99-002" })
  await render()
  await openCard(/^Captain Marlow/)

  await press(button("Remove", sheet()))
  expect(
    sheet().querySelector('[aria-label="Remove card"]')?.textContent
  ).toContain("Alex’s Binder")
  await press(button("Keep it", sheet()))
  expect((await stored()).entries).toHaveLength(2)
  expect(sheet().querySelector('[aria-label="Remove card"]')).toBeNull()

  await press(button("Remove", sheet()))
  await press(button("Remove for good", sheet()))
  expect((await stored()).entries.map((entry) => entry.card.id)).toEqual([
    "OP99-002",
  ])
  expect(document.querySelector(".sheet")).toBeNull()
  expect(cards()).toEqual(["OP99-002"])
  expect((await readCardImage({ card: "OP99-001" })).status).toBe(200)
})

test("a failed edit or removal keeps the sheet open with its error and the draft, and the retry saves", async () => {
  const alex = await owner("Alex")
  await add(alex, { card: "OP99-001" }, { card: "OP99-001", language: "ja" })
  await render()
  await openCard(/^Captain Marlow \(001\), OP99-001$/)

  vi.stubEnv("API_KEY", "synthetic-rejected-key")
  await type(field("Quantity", sheet()), "4")
  await press(button("Save changes", sheet()))
  expect(sheet().querySelector(".list-failure")?.textContent).toContain(
    "OpenHome rejected the API key"
  )
  expect(sheet().textContent).not.toContain("synthetic-rejected-key")
  expect(field("Quantity", sheet()).value).toBe("4")
  vi.stubEnv("API_KEY", key)
  expect((await stored()).entries.map((entry) => entry.quantity)).toEqual([
    1, 1,
  ])

  await press(button("Remove", sheet()))
  vi.stubEnv("API_KEY", "synthetic-rejected-key")
  await press(button("Remove for good", sheet()))
  expect(sheet().querySelector(".list-failure")).not.toBeNull()
  expect(sheet().querySelector('[aria-label="Remove card"]')).not.toBeNull()
  vi.stubEnv("API_KEY", key)
  expect(await stored().then((data) => data.entries)).toHaveLength(2)
  await press(button("Keep it", sheet()))

  await press(button("JP", sheet()))
  await press(button("Save changes", sheet()))
  expect(sheet().querySelector(".list-failure")?.textContent).toContain(
    "already holds the card in that language"
  )
  expect(button("JP", sheet()).getAttribute("aria-pressed")).toBe("true")
  expect(
    (await stored()).entries.map((entry) => [entry.language, entry.quantity])
  ).toEqual([
    ["en", 1],
    ["ja", 1],
  ])

  await press(button("EN", sheet()))
  await type(field("Quantity", sheet()), "4")
  await press(button("Save changes", sheet()))
  expect(sheet().querySelector(".list-failure")).toBeNull()
  expect((await stored()).entries[0]).toMatchObject({ quantity: 4 })
})

test("a removed Household User takes their Binder with them while the cards stay", async () => {
  const alex = await owner("Alex")
  const robin = await owner("Robin")
  await add(alex, { card: "OP99-001" })
  await add(robin, { card: "OP99-001" })
  await writeWishlist({ kind: "remove-owner", owner_id: alex })

  const data = await stored()

  expect(data.entries.map((entry) => entry.owner_id)).toEqual([robin])
  expect((await readCardImage({ card: "OP99-001" })).status).toBe(200)
})
