// @vitest-environment happy-dom
// @vitest-environment-options {"settings":{"fetch":{"disableSameOriginPolicy":true}}}
// The private server transport calls Axum; browser CORS applies only to Start RPCs.
import { once } from "node:events"
import {
  createServer,
  type Server,
  type IncomingMessage,
  type ServerResponse,
} from "node:http"
import { act } from "react"
import {
  createMemoryHistory,
  createRootRoute,
  createRouter,
  RouterContextProvider,
} from "@tanstack/react-router"
import { z } from "zod"
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
import { Programme } from "@/components/programme"
import type { PersonSearch } from "@/lib/lists"
import {
  readWatchlists,
  writeWatchlist,
  searchWatchlistMedia,
} from "@/lib/watchlist.server"
import { writeWishlist } from "@/lib/wishlist.server"
import type { WatchlistMutation, MediaSearchResult } from "@/lib/watchlist"
import { startAxumFixture } from "./support/axum-fixture"
import { button, field, settle, sheet, type } from "./support/lists-dom"

// TMDB is the fixture's loopback stub: it knows "A Quiet Orbit" and "The Lighthouse Journey".
let fixture: Awaited<ReturnType<typeof startAxumFixture>>

let root: Root

let container: HTMLDivElement

let work: Promise<WatchlistMutation> = Promise.resolve({ state: "saved" })

let searchWork: Promise<MediaSearchResult> = Promise.resolve({
  state: "unavailable",
})

let refreshWork: Promise<void> = Promise.resolve()

let search: PersonSearch = {}

let proxy: Server | undefined

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
  search = {}
  container = document.createElement("div")
  document.body.append(container)
  root = createRoot(container)
  work = Promise.resolve({ state: "saved" })
  refreshWork = Promise.resolve()
})

afterEach(async () => {
  await act(async () => root.unmount())
  container.remove()

  if (proxy) {
    proxy.closeAllConnections()
    await new Promise<void>((resolve, reject) =>
      proxy?.close((error) => (error ? reject(error) : resolve()))
    )
    proxy = undefined
  }

  vi.stubEnv("OPENHOME_API_URL", fixture.url)
  vi.stubEnv("API_KEY", "synthetic-dashboard-api-key")
  const data = await readWatchlists()

  if (data.state !== "available") throw new Error("Expected fixture discovery")

  for (const owner of data.owners)
    await writeWishlist({ kind: "remove-owner", owner_id: owner.id })
  vi.unstubAllEnvs()
  vi.unstubAllGlobals()
})

async function render(): Promise<void> {
  const data = await readWatchlists()

  const router = createRouter({
    routeTree: createRootRoute(),
    history: createMemoryHistory(),
    isServer: false,
  })

  await act(async () =>
    root.render(
      <RouterContextProvider router={router}>
        <Programme
          data={data}
          search={search}
          choose={(next) => {
            search = next
            refreshWork = render()
          }}
          find={(input) => {
            searchWork = searchWatchlistMedia(input)

            return searchWork
          }}
          change={(command) => {
            work = writeWatchlist(command)

            return work
          }}
          refresh={() => {
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
    await searchWork
    await work
    await refreshWork
  })
  await settle()
}

async function click(
  name: string | RegExp,
  scope: ParentNode = document
): Promise<void> {
  await press(button(name, scope))
}

/** Open the add plate and search; the lookup waits for typing to pause. */
async function find(query: string, expected: string | RegExp): Promise<void> {
  await click("Add a title", container)
  await type(field("Find a movie or series", sheet()), query)
  await debounced()
  expect(sheet().textContent).toMatch(expected)
}

/** Wait out the search debounce, then the lookup it started. */
async function debounced(): Promise<void> {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 450))
    await searchWork
  })
  await settle()
}

async function open(title: string): Promise<void> {
  const plate = Array.from(
    container.querySelectorAll<HTMLButtonElement>(".programme-title")
  ).find((element) => element.textContent?.includes(title))

  if (plate === undefined) throw new Error(`Missing title ${title}`)
  await press(plate)
}

async function available() {
  const data = await readWatchlists()

  if (data.state !== "available")
    throw new Error("Expected authenticated Watchlist read")

  return data
}

async function owners(...names: string[]) {
  for (const name of names) await writeWishlist({ kind: "create-owner", name })

  return (await available()).owners
}

test("adds a TMDB title and a hand-written one for the chosen person, and deletes only after confirmation", async () => {
  await render()
  expect(container.textContent).toContain(
    "Household members are added under Wish."
  )
  await owners("Member A", "Member B")
  await render()

  await find("Orbit", "1 title found")
  await click(/^A Quiet Orbit/, sheet())
  expect(sheet().textContent).toContain("For whom?")
  await click("Member A", sheet())

  await find("A Paper Garden", "Nothing found on TMDB")
  await click("Series", sheet())
  await click("Member B", sheet())

  const entries = (await available()).entries

  expect(entries).toHaveLength(2)

  const orbit = entries.find((entry) => entry.title === "A Quiet Orbit")
  const garden = entries.find((entry) => entry.title === "A Paper Garden")

  expect(orbit).toMatchObject({
    kind: "movie",
    tmdb_id: 42,
    status: "planned",
    release_date: "2020-02-29",
  })
  expect(garden).toMatchObject({
    kind: "series",
    tmdb_id: null,
    status: "planned",
  })
  expect(
    container.querySelector('section[aria-label="Planned"]')?.textContent
  ).toContain("A Paper Garden")
  expect(
    container
      .querySelector<HTMLImageElement>(".programme-title img")
      ?.getAttribute("src")
  ).toBe("https://image.tmdb.org/t/p/w342/synthetic.jpg")
  expect(
    container.querySelector('[aria-label="TMDB credits"]')?.textContent
  ).toContain("not endorsed or certified by TMDB")

  await open("A Quiet Orbit")
  await click("Delete", sheet())
  expect(
    sheet().querySelector('[aria-label="Delete entry"]')?.textContent
  ).toContain("Member A’s watchlist")
  await click("Keep it", sheet())
  expect((await available()).entries).toHaveLength(2)
  await click("Delete", sheet())
  await click("Delete for good", sheet())
  expect(
    Array.from(
      container.querySelectorAll(".programme-name"),
      (name) => name.textContent
    )
  ).toEqual(["A Paper Garden"])
  expect((await available()).entries).toEqual([garden])
})

test("status keys and date corrections persist, failures say so, and the person keys narrow the programme", async () => {
  const [memberA, memberB] = await owners("Member A", "Member B")

  if (memberA === undefined || memberB === undefined)
    throw new Error("Expected owners")

  await writeWatchlist({
    action: "add",
    owner_id: memberA.id,
    kind: "series",
    title: "A Quiet Orbit",
    request_id: crypto.randomUUID(),
  })
  await writeWatchlist({
    action: "add",
    owner_id: memberB.id,
    kind: "movie",
    title: "The Lighthouse Journey",
    request_id: crypto.randomUUID(),
  })
  await render()
  await open("A Quiet Orbit")
  await click("Watching", sheet())
  expect(
    container.querySelector('section[aria-label="Watching now"]')?.textContent
  ).toContain("A Quiet Orbit")

  const watching = (await available()).entries.find(
    (entry) => entry.title === "A Quiet Orbit"
  )

  expect(watching).toMatchObject({ status: "watching", completion_date: null })
  await click("Completed", sheet())
  expect(
    (await available()).entries.find((entry) => entry.id === watching?.id)
      ?.completion_date
  ).toMatch(/^\d{4}-\d{2}-\d{2}$/)

  await click("Change dates", sheet())
  await type(field("Added", sheet()), "2024-02-28")
  await type(field("Completed", sheet()), "2024-02-29")
  await click("Save dates", sheet())
  expect(
    (await available()).entries.find((entry) => entry.id === watching?.id)
  ).toMatchObject({
    status: "completed",
    kind: "series",
    date_added: "2024-02-28",
    completion_date: "2024-02-29",
  })
  expect(
    container.querySelector('section[aria-label="Completed"]')?.textContent
  ).toContain("29 Feb 2024")

  vi.stubEnv("API_KEY", "synthetic-rejected-key")
  await click("Planned", sheet())
  expect(sheet().querySelector(".list-failure")?.textContent).toContain(
    "OpenHome rejected the API key"
  )
  expect(sheet().textContent).not.toContain("synthetic-rejected-key")
  vi.stubEnv("API_KEY", "synthetic-dashboard-api-key")
  expect(
    (await available()).entries.find((entry) => entry.id === watching?.id)
      ?.status
  ).toBe("completed")
  await click("Close", sheet())

  await click(/^Member B/, container)
  expect(search).toEqual({ person: memberB.id })
  expect(button(/^Member B/, container).getAttribute("aria-pressed")).toBe(
    "true"
  )
  expect(container.textContent).toContain("The Lighthouse Journey")
  expect(container.textContent).not.toContain("A Quiet Orbit")
  await click("Everyone", container)
  expect(container.textContent).toContain("A Quiet Orbit")

  vi.stubEnv("API_KEY", "synthetic-rejected-key")
  await render()
  expect(container.textContent).toContain("Watchlists unavailable")
  expect(container.textContent).not.toContain("Nothing planned yet")
})

test("a lost addition keeps its person, kind and request ID across a remount", async () => {
  await owners("Member A")
  let drop = true
  const requests: string[] = []

  async function forward(
    request: IncomingMessage,
    response: ServerResponse
  ): Promise<void> {
    const body = await new Promise<string>((resolve) => {
      let text = ""
      request.on("data", (chunk: Buffer) => {
        text += chunk.toString()
      })
      request.on("end", () => resolve(text))
    })

    const upstream = await fetch(`${fixture.url}${request.url ?? ""}`, {
      method: request.method ?? "GET",
      headers: {
        Authorization: request.headers.authorization ?? "",
        "Content-Type": "application/json",
      },
      body: body || null,
    })

    if (request.url === "/api/watchlist/entries" && request.method === "POST") {
      requests.push(body)

      if (drop) {
        drop = false
        await upstream.text()
        request.socket.destroy()

        return
      }
    }

    response.writeHead(upstream.status, { "Content-Type": "application/json" })
    response.end(await upstream.text())
  }

  proxy = createServer((request, response) => {
    void forward(request, response).catch(() => response.destroy())
  })
  proxy.listen(0, "127.0.0.1")
  await once(proxy, "listening")
  const address = z.object({ port: z.number() }).parse(proxy.address())
  vi.stubEnv("OPENHOME_API_URL", `http://127.0.0.1:${address.port}`)
  await render()
  await find("\u0085A Paper Garden\u0085", "Nothing found on TMDB")
  await click("Series", sheet())
  await click("Member A", sheet())
  expect(sheet().textContent).toContain("The response was lost")

  await act(async () => root.unmount())
  root = createRoot(container)
  await render()
  await find("A Paper Garden", "Nothing found on TMDB")
  await click("Series", sheet())
  await click("Member A", sheet())
  expect(requests).toHaveLength(2)
  expect(requests[0]).toBe(requests[1])
  expect((await available()).entries).toMatchObject([
    { title: "A Paper Garden", kind: "series" },
  ])
  expect(sessionStorage.length).toBe(0)
})

test("TMDB outages leave hand planning open, and found details keep a completed title's dates", async () => {
  const [member] = await owners("Member A")

  if (member === undefined) throw new Error("Expected owner")

  await render()
  await find("Offline story", "TMDB is not answering")
  await click("Movie", sheet())
  await click("Member A", sheet())
  expect(
    (await available()).entries.find((entry) => entry.title === "Offline story")
  ).toMatchObject({ tmdb_id: null, kind: "movie", status: "planned" })

  const added = await writeWatchlist({
    action: "add",
    owner_id: member.id,
    kind: "movie",
    title: "A Quiet Orbit",
    request_id: crypto.randomUUID(),
  })

  expect(added.state).toBe("saved")

  const original = (await available()).entries.find(
    (entry) => entry.title === "A Quiet Orbit"
  )

  if (original === undefined) throw new Error("Expected manual plan")
  await writeWatchlist({
    action: "edit",
    owner_id: member.id,
    entry_id: original.id,
    status: "completed",
    date_added: "2024-02-28",
    completion_date: "2024-02-29",
  })
  await render()
  await open("A Quiet Orbit")
  await click("Find details", sheet())
  await debounced()
  await click(
    /^A Quiet Orbit/,
    sheet().querySelector(".media-results") ?? sheet()
  )
  expect(sheet().textContent).toContain("A fictional journey")
  expect(
    (await available()).entries.find((e) => e.id === original.id)
  ).toMatchObject({
    tmdb_id: 42,
    status: "completed",
    date_added: "2024-02-28",
    completion_date: "2024-02-29",
  })
})
