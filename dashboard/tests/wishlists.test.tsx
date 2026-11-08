// @vitest-environment happy-dom
// @vitest-environment-options {"settings":{"fetch":{"disableSameOriginPolicy":true}}}
// This harness also executes the private server transport; CORS applies to browser RPCs, not Axum calls.

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
import { GiftBoard } from "@/components/gift-board"
import type { WishSearch } from "@/lib/lists"
import {
  readWishlists,
  writeWishlist,
  previewWishlistUrl,
} from "@/lib/wishlist.server"
import type {
  WishlistCommand,
  WishlistState,
  WishlistMutation,
  WishlistMetadataLoader,
} from "@/lib/wishlist"
import { readWatchlists, writeWatchlist } from "@/lib/watchlist.server"
import { startAxumFixture } from "./support/axum-fixture"
import { startLossyAxum } from "./support/lost-response"
import { button, field, settle, sheet, type } from "./support/lists-dom"

let fixture: Awaited<ReturnType<typeof startAxumFixture>>

let apiUrl: string

let root: Root

let container: HTMLDivElement

const proxies: Array<() => Promise<void>> = []

let mutationWork: Promise<WishlistMutation> = Promise.resolve({
  state: "saved",
})

let refreshWork: Promise<void> = Promise.resolve()

let metadataWork: Promise<unknown> = Promise.resolve()

let search: WishSearch = {}

let metadata: WishlistMetadataLoader = previewWishlistUrl

let refreshFails = false

let sent: WishlistCommand[] = []

/** While set, a write waits for it before reaching Axum. */
let held: Promise<void> | undefined

beforeAll(async () => {
  fixture = await startAxumFixture()
  apiUrl = fixture.url
})

afterAll(async () => {
  await fixture.stop()
})

beforeEach(() => {
  vi.stubEnv("OPENHOME_API_URL", apiUrl)
  vi.stubEnv("API_KEY", "synthetic-dashboard-api-key")
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  sessionStorage.clear()
  search = {}
  metadata = previewWishlistUrl
  refreshFails = false
  sent = []
  held = undefined
  container = document.createElement("div")
  document.body.append(container)
  root = createRoot(container)
  mutationWork = Promise.resolve({ state: "saved" })
  refreshWork = Promise.resolve()
  metadataWork = Promise.resolve()
})

afterEach(async () => {
  await act(async () => root.unmount())
  container.remove()
  vi.unstubAllGlobals()
  vi.unstubAllEnvs()

  for (const stop of proxies.splice(0)) await stop()

  // Clean through the public API; receipts intentionally remain retired.
  const response = await fetch(`${apiUrl}/api/household-users`, {
    headers: { Authorization: "Bearer synthetic-dashboard-api-key" },
  })

  const owners = z
    .array(z.object({ id: z.uuid() }))
    .parse(await response.json())

  for (const owner of owners)
    await fetch(`${apiUrl}/api/household-users/${owner.id}`, {
      method: "DELETE",
      headers: { Authorization: "Bearer synthetic-dashboard-api-key" },
    })
})

async function render(): Promise<void> {
  const data = await readWishlists()

  const router = createRouter({
    routeTree: createRootRoute(),
    history: createMemoryHistory(),
    isServer: false,
  })

  await act(async () => {
    root.render(
      <RouterContextProvider router={router}>
        <GiftBoard
          data={data}
          search={search}
          choose={(next) => {
            search = next
            refreshWork = render()
          }}
          change={(command) => {
            sent.push(command)
            mutationWork =
              held === undefined
                ? writeWishlist(command)
                : held.then(() => writeWishlist(command))

            return mutationWork
          }}
          refresh={() => {
            if (refreshFails)
              return Promise.reject(new Error("Synthetic outage"))

            refreshWork = render()

            return refreshWork
          }}
          metadata={(request) => {
            const work = metadata(request)
            metadataWork = work

            return work
          }}
        />
      </RouterContextProvider>
    )
  })
}

async function press(element: HTMLButtonElement): Promise<void> {
  await act(async () => {
    element.click()
    await mutationWork
    await refreshWork
    await metadataWork
  })
  await settle()
}

async function click(
  name: string | RegExp,
  scope: ParentNode = document
): Promise<void> {
  await press(button(name, scope))
}

/** Wait out the link debounce, then the read it started. */
async function debounced(): Promise<void> {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 500))
    await metadataWork
  })
  await settle()
}

/** An input in the open sheet, found by its current value. */
function valued(value: string, scope: ParentNode = sheet()): HTMLInputElement {
  const input = Array.from(scope.querySelectorAll("input")).find(
    (element) => element.value === value
  )

  if (input === undefined) throw new Error(`Missing input ${value}`)

  return input
}

function section(label: string, scope: ParentNode = sheet()): HTMLElement {
  const element = scope.querySelector<HTMLElement>(
    `section[aria-label="${label}"]`
  )

  if (element === null) throw new Error(`Missing section ${label}`)

  return element
}

async function plate(title: string): Promise<void> {
  const match = Array.from(
    container.querySelectorAll<HTMLButtonElement>(".wish-plate")
  ).find((element) => element.textContent?.includes(title))

  if (match === undefined) throw new Error(`Missing plate ${title}`)
  await press(match)
}

function titles(scope: ParentNode = container): string[] {
  return Array.from(
    scope.querySelectorAll(".wish-title"),
    (element) => element.textContent ?? ""
  )
}

async function available(): Promise<
  Extract<WishlistState, { state: "available" }>
> {
  const data = await readWishlists()

  if (data.state !== "available")
    throw new Error("Expected authenticated Wishlist read")

  return data
}

/** A person with named lists, made through the API. */
async function household(people: Record<string, string[]>) {
  for (const [name, lists] of Object.entries(people)) {
    await writeWishlist({ kind: "create-owner", name })
    const owner = (await available()).owners.find((item) => item.name === name)

    if (owner === undefined) throw new Error("Expected owner")

    for (const list of lists)
      await writeWishlist({
        kind: "create-list",
        owner_id: owner.id,
        name: list,
      })
  }

  const data = await available()

  return {
    owner: (name: string) => {
      const owner = data.owners.find((item) => item.name === name)

      if (owner === undefined) throw new Error(`Missing owner ${name}`)

      return owner
    },
    list: (ownerName: string, name: string) => {
      const owner = data.owners.find((item) => item.name === ownerName)

      const list = data.lists.find(
        (item) => item.owner_id === owner?.id && item.name === name
      )

      if (list === undefined) throw new Error(`Missing list ${name}`)

      return list
    },
  }
}

/** Axum commits the next wish addition, but its response never reaches the Dashboard. */
async function loseNextAddition(): Promise<void> {
  const proxy = await startLossyAxum(
    apiUrl,
    (request) => request.url === "/api/wishlist/items"
  )

  proxies.push(proxy.stop)
  vi.stubEnv("OPENHOME_API_URL", proxy.url)
}

/** Browser storage that refuses every call, as in private browsing. */
function withoutSessionStorage(): void {
  const refuse = () => {
    throw new DOMException("Storage is disabled", "SecurityError")
  }

  vi.stubGlobal("sessionStorage", {
    getItem: refuse,
    setItem: refuse,
    removeItem: refuse,
    clear: refuse,
  })
}

async function wish(
  list: { id: string; owner_id: string },
  title: string,
  extra: {
    priority?: "low" | "normal" | "high"
    price?: string
    quantity?: number
  } = {}
): Promise<void> {
  const saved = await writeWishlist({
    kind: "add-item",
    owner_id: list.owner_id,
    list_id: list.id,
    title,
    request_id: crypto.randomUUID(),
    ...extra,
  })

  expect(saved.state).toBe("saved")
}

test("the household sheet adds, renames and removes people and lists, confirming every removal", async () => {
  await render()
  expect(container.textContent).toContain("No one here yet")
  await click("Add a household member", container)
  await type(field("New household member", sheet()), "Member A")
  await click("New household member", sheet())
  await type(field("New household member", sheet()), "Member B")
  await click("New household member", sheet())
  expect(field("New household member", sheet()).value).toBe("")

  for (const [owner, list] of [
    ["Member A", "Ideas"],
    ["Member A", "Outdoors"],
    ["Member B", "Ideas"],
  ] as const) {
    await type(field(`New list for ${owner}`, sheet()), list)
    await click(`New list for ${owner}`, sheet())
  }

  const memberA = section("Member A")
  const ideas = valued("Ideas", memberA)
  await type(ideas, "Outdoors")
  await click("Save list name", memberA)
  expect(sheet().querySelector(".list-failure")?.textContent).toContain(
    "This name or request is already in use"
  )
  expect(ideas.value).toBe("Outdoors")
  await type(ideas, "Clothes")
  await click("Save list name", memberA)

  const data = await available()
  const clothes = data.lists.find((list) => list.name === "Clothes")

  if (clothes === undefined) throw new Error("Expected renamed list")
  expect(data.lists.map((list) => list.name).sort()).toEqual([
    "Clothes",
    "Ideas",
    "Outdoors",
  ])

  await wish(clothes, "A waterproof jacket")
  await render()
  await click("Remove Clothes", section("Member A"))
  expect(
    section("Member A").querySelector('[aria-label="Remove Clothes"]')
      ?.textContent
  ).toContain("the 1 wish in it")
  await click("Keep", section("Member A"))
  expect((await available()).items).toHaveLength(1)
  await click("Remove Clothes", section("Member A"))
  await click("Remove for good", section("Member A"))
  expect((await available()).items).toEqual([])

  await type(valued("Member A"), "Member C")
  await click("Save name", section("Member A"))
  expect(sheet().textContent).toContain("Member C")

  const member = (await available()).owners.find(
    (owner) => owner.name === "Member C"
  )

  if (member === undefined) throw new Error("Expected Member C")

  await writeWatchlist({
    action: "add",
    owner_id: member.id,
    kind: "movie",
    title: "A Quiet Orbit",
    request_id: crypto.randomUUID(),
  })
  await click("Remove Member C", section("Member C"))
  expect(
    section("Member C").querySelector('[aria-label="Remove Member C"]')
      ?.textContent
  ).toContain("wishlists, wishes and watchlist entries")
  await click("Keep", section("Member C"))

  const retained = await readWatchlists()

  expect(
    retained.state === "available" ? retained.entries.length : undefined
  ).toBe(1)
  await click("Remove Member C", section("Member C"))
  await click("Remove for good", section("Member C"))

  const remaining = await readWatchlists()

  expect(
    remaining.state === "available" ? remaining.entries : undefined
  ).toEqual([])
  expect((await available()).owners.map((owner) => owner.name)).toEqual([
    "Member B",
  ])
})

test("plates group by person and list, the keys filter them, and the detail plate edits and deletes", async () => {
  const home = await household({
    "Member A": ["Ideas", "Outdoors"],
    "Member B": ["Ideas"],
  })

  await wish(home.list("Member A", "Ideas"), "A scarf")
  await wish(home.list("Member A", "Ideas"), "A warm jacket", {
    priority: "high",
    price: "DKK 199.95",
    quantity: 2,
  })
  await wish(home.list("Member A", "Outdoors"), "A tent")
  await wish(home.list("Member B", "Ideas"), "A book")
  await render()

  // Everyone: one group per person, high priority first, each plate captioned by its list.
  expect(titles(section("Member A", container))).toEqual([
    "A warm jacket",
    "A scarf",
    "A tent",
  ])
  expect(titles(section("Member B", container))).toEqual(["A book"])
  expect(section("Member A", container).textContent).toContain("Outdoors")
  expect(section("Member A", container).textContent).toContain("×2")

  await click(/^Member A/, container)
  expect(search).toEqual({ person: home.owner("Member A").id, list: undefined })
  expect(titles()).not.toContain("A book")
  expect(titles(section("Outdoors", container))).toEqual(["A tent"])
  await click(
    /^Outdoors/,
    container.querySelector('[aria-label="Member A’s lists"]') ?? container
  )
  expect(search.list).toBe(home.list("Member A", "Outdoors").id)
  expect(titles()).toEqual(["A tent"])

  await plate("A tent")
  await click("Edit", sheet())
  await type(field("Title", sheet()), "A bigger tent")
  await type(field("Price", sheet()), "DKK 999")
  await type(field("Link", sheet()), "https://example.com/tent")
  await type(field("Notes", sheet()), "For summer")
  await click("High", sheet())
  await click("One more", sheet())
  await click("Save", sheet())

  const item = (await available()).items.find(
    (entry) => entry.title === "A bigger tent"
  )

  expect(item).toMatchObject({
    url: "https://example.com/tent",
    notes: "For summer",
    price: "DKK 999",
    quantity: 2,
    priority: "high",
  })
  expect(
    sheet().querySelector('a[href="https://example.com/tent"]')?.textContent
  ).toContain("example.com")

  await click("Edit", sheet())
  await type(field("Link", sheet()), "javascript:alert(1)")
  // Dispatch at the form seam so Axum validation runs even where native URL checks would stop it.
  await act(async () => {
    sheet()
      .querySelector("form")
      ?.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }))
    await mutationWork
    await refreshWork
  })
  expect(sheet().querySelector(".list-failure")?.textContent).toContain(
    "Check the fields"
  )
  expect(field("Link", sheet()).value).toBe("javascript:alert(1)")
  expect(
    (await available()).items.find((entry) => entry.id === item?.id)
  ).toEqual(item)
  await type(field("Link", sheet()), "")
  await type(field("Price", sheet()), "")
  await click("Save", sheet())
  expect(
    (await available()).items.find((entry) => entry.id === item?.id)
  ).toMatchObject({
    url: null,
    price: null,
    notes: "For summer",
    quantity: 2,
    priority: "high",
  })

  await click("Delete", sheet())
  expect(
    sheet().querySelector('[aria-label="Delete wish"]')?.textContent
  ).toContain("Outdoors")
  await click("Keep it", sheet())
  expect((await available()).items).toHaveLength(4)
  await click("Delete", sheet())
  await click("Delete for good", sheet())
  expect((await available()).items).toHaveLength(3)
  expect(titles()).toEqual([])
  expect(container.textContent).toContain("Add the first wish")

  vi.stubEnv("API_KEY", "synthetic-rejected-key")
  await render()
  expect(container.textContent).toContain("Wishlists unavailable")
  expect(container.textContent).not.toContain("synthetic-rejected-key")
})

test("a pasted link fills editable fields for the chosen list, and nothing saves before Add wish", async () => {
  const home = await household({ "Member A": ["Ideas"] })
  const list = home.list("Member A", "Ideas")

  // A protocol fixture exercises review in the UI; extraction is owned by the Axum tests.
  metadata = async (request) => {
    const { url, ...destination } = request

    expect(destination).toEqual({ owner_id: list.owner_id, list_id: list.id })
    expect(url).toMatch(/^https:\/\/example\.com\/(product|another-product)$/)

    return {
      state: "preview",
      data: {
        status: "ready",
        title: url.endsWith("another-product")
          ? "Suggested hat"
          : "Suggested coat",
        url,
        image_url: "https://example.com/coat.png",
        price: "DKK 199.95",
      },
    }
  }

  search = { person: list.owner_id, list: list.id }
  await render()
  await click("Add a wish", container)
  expect(button("Ideas", sheet()).getAttribute("aria-pressed")).toBe("true")
  await type(field("Paste a link", sheet()), "https://example.com/product")
  await debounced()
  expect(field("Title", sheet()).value).toBe("Suggested coat")
  expect(field("Image link", sheet()).value).toBe(
    "https://example.com/coat.png"
  )
  expect(field("Price", sheet()).value).toBe("DKK 199.95")
  expect(sheet().textContent).toContain("Read from the link")
  expect(sheet().querySelector("img")).toBeNull()
  expect((await available()).items).toEqual([])

  await type(
    field("Paste a link", sheet()),
    "https://example.com/another-product"
  )
  expect(field("Title", sheet()).value).toBe("")
  expect(field("Price", sheet()).value).toBe("")
  await debounced()
  expect(field("Title", sheet()).value).toBe("Suggested hat")
  await type(field("Paste a link", sheet()), "https://example.com/product")
  await debounced()
  await type(field("Image link", sheet()), "https://example.com/reviewed.png")
  await type(field("Price", sheet()), "DKK 179.95")
  await type(field("Title", sheet()), "My coat")
  await click("Add wish", sheet())
  expect((await available()).items[0]).toMatchObject({
    title: "My coat",
    list_id: list.id,
    url: "https://example.com/product",
    image_url: "https://example.com/reviewed.png",
    price: "DKK 179.95",
  })
  const [saved] = (await available()).items
  // The browser never loads the shop: the picture travels through the dashboard's relay.
  expect(
    container.querySelector(".wish-plate img")?.getAttribute("src")
  ).toMatch(
    new RegExp(
      `^/wish-image/${list.owner_id}/${list.id}/${saved?.id}\\?v=\\w+$`
    )
  )

  metadata = async () => ({
    state: "preview",
    data: {
      status: "partial",
      title: "Suggested title",
      url: "https://example.com/other",
      image_url: null,
      price: null,
    },
  })
  await click("Add a wish", container)
  await type(field("Title", sheet()), "My own idea")
  await type(field("Paste a link", sheet()), "https://example.com/other")
  await debounced()
  expect(sheet().textContent).toContain("Partly read from the link")
  expect(field("Title", sheet()).value).toBe("My own idea")
  await click("Add wish", sheet())
  expect((await available()).items).toHaveLength(2)
})

test("an unreadable link keeps the manual draft, and the link is only read once a list is chosen", async () => {
  const home = await household({ "Member A": ["Ideas"] })
  await render()
  await click("Add a wish", container)
  await type(field("Paste a link", sheet()), "http://127.0.0.1/product")
  await debounced()
  expect(sheet().textContent).toContain("Choose who it is for")
  await click("Member A", sheet())
  await click("Ideas", sheet())
  await debounced()
  expect(sheet().textContent).toContain("Could not read this link")
  expect(field("Paste a link", sheet()).value).toBe("http://127.0.0.1/product")
  expect((await available()).items).toEqual([])
  // Tests never reach the internet: the next link is read by a stub that cannot answer.
  metadata = () => Promise.reject(new Error("Synthetic outage"))
  await type(field("Paste a link", sheet()), "https://example.com/product")
  await type(field("Title", sheet()), "A manually described coat")
  await click("Add wish", sheet())
  expect((await available()).items[0]).toMatchObject({
    title: "A manually described coat",
    list_id: home.list("Member A", "Ideas").id,
    url: "https://example.com/product",
  })
})

test.each([
  {
    title: " A book ",
    retry: "A book",
    expected: ["A book"],
    legacy: false,
    rich: false,
  },
  {
    title: " A book ",
    retry: "A book",
    expected: ["A book"],
    legacy: true,
    rich: false,
  },
  {
    title: "A book",
    retry: "A book",
    expected: ["A book"],
    legacy: false,
    rich: true,
  },
  {
    title: "\u0085A book\u0085",
    retry: "A book",
    expected: ["A book"],
    legacy: false,
    rich: false,
  },
  {
    title: "﻿A book﻿",
    retry: "A book",
    expected: ["A book", "﻿A book﻿"],
    legacy: false,
    rich: false,
  },
])(
  "a lost addition respects API title identity after remount ($title), and auth failures retain the draft",
  async ({ title, retry, expected, legacy, rich }) => {
    const home = await household({ "Member A": ["Ideas"] })
    const list = home.list("Member A", "Ideas")
    await loseNextAddition()
    search = { person: list.owner_id, list: list.id }
    await render()
    await click("Add a wish", container)

    if (rich) {
      await type(field("Price", sheet()), "DKK 199.95")
      await click("One more", sheet())
    }

    await type(field("Title", sheet()), title)
    await click("Add wish", sheet())
    expect(sheet().textContent).toContain("The response was lost")

    if (legacy) {
      const key = `openhome.pending-wish.${list.id}`

      const prior = z
        .object({ ownerId: z.uuid(), listId: z.uuid(), requestId: z.uuid() })
        .parse(JSON.parse(sessionStorage.getItem(key) ?? "null"))

      sessionStorage.setItem(key, JSON.stringify({ ...prior, title }))
    }

    await act(async () => root.unmount())
    root = createRoot(container)
    await render()
    await click("Add a wish", container)

    if (rich) {
      expect(field("Price", sheet()).value).toBe("DKK 199.95")
      expect(sheet().querySelector(".stepper output")?.textContent).toBe("2")
    }

    await type(field("Title", sheet()), retry)
    await click("Add wish", sheet())
    expect((await available()).items.map((item) => item.title).sort()).toEqual(
      expected
    )
    expect(document.querySelector(".sheet")).toBeNull()

    await click("Add a wish", container)
    vi.stubEnv("API_KEY", "synthetic-rejected-key")
    await type(field("Title", sheet()), "A rain hat")
    await click("Add wish", sheet())
    expect(sheet().textContent).toContain("OpenHome rejected the API key")
    expect(sheet().textContent).not.toContain("synthetic-rejected-key")
    expect(field("Title", sheet()).value).toBe("A rain hat")
    vi.stubEnv("API_KEY", "synthetic-dashboard-api-key")
    await click("Add wish", sheet())
    expect((await available()).items.map((item) => item.title).sort()).toEqual(
      [...expected, "A rain hat"].sort()
    )
  }
)

/** The wish additions the board has sent so far, in order. */
function additions() {
  return sent.flatMap((command) =>
    command.kind === "add-item" ? [command] : []
  )
}

test("a saved wish stays acknowledged when the view cannot be re-read, and its retry intention is spent", async () => {
  const home = await household({ "Member A": ["Ideas"] })
  const list = home.list("Member A", "Ideas")

  search = { person: list.owner_id, list: list.id }
  await render()
  await click("Add a wish", container)
  await type(field("Title", sheet()), "A book")
  refreshFails = true
  await click("Add wish", sheet())

  expect((await available()).items.map((item) => item.title)).toEqual([
    "A book",
  ])
  // The add is done: the sheet closed, and the failure names the view, not the save.
  expect(document.querySelector(".sheet")).toBeNull()
  expect(container.querySelector(".list-failure")?.textContent).toContain(
    "Saved to OpenHome, but the view could not update"
  )
  expect(titles()).toEqual([])
  expect(sessionStorage.getItem(`openhome.pending-wish.${list.id}`)).toBeNull()

  refreshFails = false
  await render()
  expect(titles()).toEqual(["A book"])
  expect(additions()).toHaveLength(1)
})

test("a rejected addition keeps its request ID and arguments for a retry after remount, and changed input gets a new one", async () => {
  const home = await household({ "Member A": ["Ideas"] })
  const list = home.list("Member A", "Ideas")

  search = { person: list.owner_id, list: list.id }
  await render()
  await click("Add a wish", container)
  await type(field("Price", sheet()), "DKK 99")
  await type(field("Title", sheet()), "A book")
  vi.stubEnv("API_KEY", "synthetic-rejected-key")
  await click("Add wish", sheet())
  expect(sheet().textContent).toContain("OpenHome rejected the API key")

  await act(async () => root.unmount())
  root = createRoot(container)
  vi.stubEnv("API_KEY", "synthetic-dashboard-api-key")
  await render()
  await click("Add a wish", container)
  // The remounted form resumes the rejected wish rather than starting empty.
  expect(field("Title", sheet()).value).toBe("A book")
  expect(field("Price", sheet()).value).toBe("DKK 99")
  await click("Add wish", sheet())

  const [first, retry] = additions()

  expect(retry).toEqual(first)
  expect((await available()).items.map((item) => item.title)).toEqual([
    "A book",
  ])

  await click("Add a wish", container)
  vi.stubEnv("API_KEY", "synthetic-rejected-key")
  await type(field("Title", sheet()), "A hat")
  await click("Add wish", sheet())
  await type(field("Title", sheet()), "A scarf")
  vi.stubEnv("API_KEY", "synthetic-dashboard-api-key")
  await click("Add wish", sheet())

  const [, , hat, scarf] = additions()

  expect(hat?.title).toBe("A hat")
  expect(scarf?.title).toBe("A scarf")
  expect(scarf?.request_id).not.toBe(hat?.request_id)
})

test("without session storage a lost addition retried in the same form repeats its request ID", async () => {
  const home = await household({ "Member A": ["Ideas"] })
  const list = home.list("Member A", "Ideas")

  await loseNextAddition()
  withoutSessionStorage()
  search = { person: list.owner_id, list: list.id }
  await render()
  await click("Add a wish", container)
  await type(field("Title", sheet()), "A book")
  await click("Add wish", sheet())
  expect(sheet().textContent).toContain("The response was lost")
  expect(field("Title", sheet()).value).toBe("A book")
  await click("Add wish", sheet())

  const [first, retry] = additions()

  expect(retry).toEqual(first)
  // Axum committed the lost request, so one wish proves the retry was recognised as the same.
  expect((await available()).items.map((item) => item.title)).toEqual([
    "A book",
  ])
  expect(document.querySelector(".sheet")).toBeNull()
})

test("a closed sheet's late save does not close the sheet opened after it", async () => {
  const home = await household({ "Member A": ["Ideas"] })
  const list = home.list("Member A", "Ideas")
  let release: () => void = () => undefined

  search = { person: list.owner_id, list: list.id }
  await render()
  await click("Add a wish", container)
  await type(field("Title", sheet()), "A book")
  held = new Promise<void>((resolve) => {
    release = resolve
  })

  // Neither click waits for the held write; the sheet may be closed while it is in flight.
  await act(async () => button("Add wish", sheet()).click())
  await act(async () => button("Close", sheet()).click())
  await settle()
  await act(async () => button("Household and lists", container).click())
  expect(sheet().textContent).toContain("Household and lists")

  await act(async () => {
    release()
    await mutationWork
    await refreshWork
  })
  await settle()

  expect((await available()).items.map((item) => item.title)).toEqual([
    "A book",
  ])
  // The write was for the first sheet; the household sheet is still the one the user opened.
  expect(sheet().textContent).toContain("Household and lists")
  // The late write was still saved, so its spent retry intention is gone.
  expect(sessionStorage.getItem(`openhome.pending-wish.${list.id}`)).toBeNull()
})
