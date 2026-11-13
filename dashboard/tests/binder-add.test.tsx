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
import {
  addBinderEntry,
  readBinder,
  searchBinderCards,
} from "@/lib/binder.server"
import type { BinderAddReceipt, CardSearchResult } from "@/lib/binder-add"
import { readWatchlists } from "@/lib/watchlist.server"
import { writeWishlist } from "@/lib/wishlist.server"
import { startAxumFixture } from "./support/axum-fixture"
import { startLossyAxum } from "./support/lost-response"
import { button, field, nameOf, settle, sheet, type } from "./support/lists-dom"

// The fixture's OPTCG stand-in knows the invented OP99 cards; OP99-503 is its outage code.
let fixture: Awaited<ReturnType<typeof startAxumFixture>>

let root: Root

let container: HTMLDivElement

let proxy: Awaited<ReturnType<typeof startLossyAxum>> | undefined

let found: Promise<CardSearchResult> = Promise.resolve({ state: "failed" })

let added: Promise<BinderAddReceipt> = Promise.resolve({ state: "saved" })

let refreshed: Promise<void> = Promise.resolve()

beforeAll(async () => {
  fixture = await startAxumFixture()
})

afterAll(async () => {
  await fixture.stop()
})

beforeEach(() => {
  vi.stubEnv("OPENHOME_API_URL", fixture.url)
  vi.stubEnv("API_KEY", "synthetic-dashboard-api-key")
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  sessionStorage.clear()
  refreshed = Promise.resolve()
  container = document.createElement("div")
  document.body.append(container)
  root = createRoot(container)
})

afterEach(async () => {
  await act(async () => root.unmount())
  container.remove()
  await proxy?.stop()
  proxy = undefined
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
          search={{}}
          choose={() => undefined}
          refresh={() => {
            refreshed = render()

            return refreshed
          }}
          find={(input) => {
            found = searchBinderCards(input)

            return found
          }}
          change={(command) => {
            added = addBinderEntry(command)

            return added
          }}
        />
      </RouterContextProvider>
    )
  )
}

async function press(element: HTMLButtonElement): Promise<void> {
  await act(async () => {
    element.click()
    await found
    await added
    await refreshed
  })
  await settle()
}

async function owners(...names: string[]) {
  for (const name of names) await writeWishlist({ kind: "create-owner", name })

  const data = await readBinder()

  if (data.state !== "available") throw new Error("Expected Binder read")

  return data.owners
}

async function stored() {
  const data = await readBinder()

  if (data.state !== "available") throw new Error("Expected Binder read")

  return data.entries.map((entry) => ({
    card: entry.card.id,
    owner: entry.owner_id,
    language: entry.language,
    quantity: entry.quantity,
  }))
}

async function seed(ownerId: string, card: string, quantity: number) {
  const response = await fetch(`${fixture.url}/api/binder/entries`, {
    method: "POST",
    headers: {
      Authorization: "Bearer synthetic-dashboard-api-key",
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      owner_id: ownerId,
      card_id: card,
      quantity,
      request_id: crypto.randomUUID(),
    }),
  })

  if (!response.ok) throw new Error(`Binder addition failed ${response.status}`)
}

/** Open the add sheet and search for a code, as a person submits it. */
async function search(code: string): Promise<void> {
  if (document.querySelector(".sheet") === null)
    await press(button("Add card", container))
  await type(field("Card Code", sheet()), code)
  await press(button("Search", sheet()))
}

const pockets = (): string[] =>
  Array.from(
    container.querySelectorAll<HTMLElement>(".pocket:not([data-empty])"),
    (pocket) => pocket.textContent ?? ""
  )

test("searches a Card Code on request and lists each variant's image and name", async () => {
  await owners("Alex")
  await render()
  await press(button("Add card", container))
  await type(field("Card Code", sheet()), "op99-001")
  await settle()
  expect(sheet().querySelector(".card-results")).toBeNull()

  await press(button("Search", sheet()))

  const variants = Array.from(
    sheet().querySelectorAll(".card-result"),
    (result) => nameOf(result)
  )

  expect(variants).toEqual([
    "Captain Marlow (001)",
    "Captain Marlow (001) (Parallel)",
  ])
  expect(
    Array.from(sheet().querySelectorAll("img"), (image) =>
      image.getAttribute("src")
    )
  ).toEqual(["/binder-image/OP99-001", "/binder-image/OP99-001_p1"])
  expect(await stored()).toEqual([])
})

test("adds the chosen variant to the selected owner as English, then shows it in the Binder", async () => {
  const [alex] = await owners("Alex", "Robin")
  await render()
  await search("OP99-001")
  await press(button("Captain Marlow (001) (Parallel)", sheet()))

  expect(sheet().textContent).toContain("Discard draft")
  expect(button("EN", sheet()).getAttribute("aria-pressed")).toBe("true")
  expect(button("Alex", sheet()).getAttribute("aria-pressed")).toBe("true")

  await press(button("Add card", sheet()))

  expect(sheet().textContent).toContain("Added to Alex’s Binder, quantity 1.")
  expect(await stored()).toEqual([
    { card: "OP99-001_p1", owner: alex?.id, language: "en", quantity: 1 },
  ])
  expect(pockets()).toHaveLength(1)
  expect(pockets()[0]).toContain("Captain Marlow (001) (Parallel)")
  expect(sessionStorage.length).toBe(0)
})

test("adds a Japanese copy for another owner, kept apart from the English one", async () => {
  const [alex, robin] = await owners("Alex", "Robin")
  await seed(robin?.id ?? "", "OP99-002", 1)
  await render()
  await search("OP99-002")
  await press(button("Wren Lanternkeeper (002)", sheet()))
  await press(button("Robin", sheet()))
  await press(button("JP", sheet()))
  await press(button("Add card", sheet()))

  expect(await stored()).toEqual([
    { card: "OP99-002", owner: robin?.id, language: "en", quantity: 1 },
    { card: "OP99-002", owner: robin?.id, language: "ja", quantity: 1 },
  ])
  expect(alex).toBeDefined()
})

test("a card already owned in the same language rises in quantity, in the sheet and the Binder", async () => {
  const [alex] = await owners("Alex")
  await seed(alex?.id ?? "", "OP99-001", 2)
  await render()
  expect(pockets()[0]).toContain("×2")

  await search("OP99-001")
  await press(button("Captain Marlow (001)", sheet()))
  await press(button("One more", sheet()))
  await press(button("Add card", sheet()))

  expect(sheet().textContent).toContain(
    "Quantity raised to 4 in Alex’s Binder."
  )
  expect(await stored()).toEqual([
    { card: "OP99-001", owner: alex?.id, language: "en", quantity: 4 },
  ])
  expect(pockets()).toHaveLength(1)
  expect(pockets()[0]).toContain("×4")
})

test("a catalogue outage says so and leaves the sheet ready for another search", async () => {
  await owners("Alex")
  await render()
  await search("OP99-503")

  expect(sheet().textContent).toContain("The card catalogue is not answering")
  expect(sheet().querySelector(".card-results")).toBeNull()

  await type(field("Card Code", sheet()), "OP99-002")
  await press(button("Search", sheet()))
  expect(sheet().querySelectorAll(".card-result")).toHaveLength(2)
  expect(sheet().textContent).not.toContain("not answering")
  expect(await stored()).toEqual([])
})

test("an unlisted code is not in the catalogue yet and offers nothing to add", async () => {
  await owners("Alex")
  await render()
  await search("OP99-777")

  expect(sheet().textContent).toContain("OP99-777 is not in the catalogue yet.")
  expect(sheet().querySelectorAll(".card-result")).toHaveLength(0)
  expect(sheet().textContent).not.toContain("Discard draft")
  expect(await stored()).toEqual([])
})

test("a malformed code is explained without a catalogue request", async () => {
  await owners("Alex")
  await render()
  await search("hello")

  expect(sheet().textContent).toContain("That is not a Card Code")
})

test("a draft survives a refresh and Discard draft clears it", async () => {
  await owners("Alex")
  await render()
  await search("OP99-001")
  await press(button("Captain Marlow (001)", sheet()))
  await press(button("One more", sheet()))

  await act(async () => root.unmount())
  root = createRoot(container)
  await render()
  await settle()

  expect(sheet().textContent).toContain("Captain Marlow (001)")
  expect(sheet().querySelector("output")?.textContent).toBe("2")
  expect(button("EN", sheet()).getAttribute("aria-pressed")).toBe("true")

  await press(button("Discard draft", sheet()))
  expect(sessionStorage.length).toBe(0)
  expect(field("Card Code", sheet())).toBeDefined()
  expect(await stored()).toEqual([])

  await act(async () => root.unmount())
  root = createRoot(container)
  await render()
  await settle()
  expect(document.querySelector(".sheet")).toBeNull()
})

test("a lost response replays the same addition after a remount, adding one entry", async () => {
  const [alex] = await owners("Alex")
  proxy = await startLossyAxum(
    fixture.url,
    (request) =>
      request.method === "POST" && request.url === "/api/binder/entries"
  )
  vi.stubEnv("OPENHOME_API_URL", proxy.url)
  await render()
  await search("OP99-001")
  await press(button("Captain Marlow (001)", sheet()))
  await press(button("Add card", sheet()))

  expect(sheet().textContent).toContain("The response was lost")
  expect(sheet().textContent).toContain("not confirmed yet")
  expect(button("One more", sheet()).disabled).toBe(true)
  expect(sessionStorage.length).toBe(1)
  expect(await stored()).toHaveLength(1)

  await act(async () => root.unmount())
  root = createRoot(container)
  await render()
  await settle()
  expect(sheet().textContent).toContain("Captain Marlow (001)")

  await press(button("Retry add", sheet()))

  expect(sheet().textContent).toContain("Added to Alex’s Binder, quantity 1.")
  expect(await stored()).toEqual([
    { card: "OP99-001", owner: alex?.id, language: "en", quantity: 1 },
  ])
  expect(sessionStorage.length).toBe(0)
})
