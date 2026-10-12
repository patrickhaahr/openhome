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
import { Watchlists } from "@/components/watchlists"
import { readWatchlists, writeWatchlist } from "@/lib/watchlist.server"
import { writeWishlist } from "@/lib/wishlist.server"
import { startAxumFixture } from "./support/axum-fixture"
import type { WatchlistFilters, WatchlistMutation } from "@/lib/watchlist"

let fixture: Awaited<ReturnType<typeof startAxumFixture>>

let root: Root

let container: HTMLDivElement

let work: Promise<WatchlistMutation> = Promise.resolve({ state: "saved" })

let refreshWork: Promise<void> = Promise.resolve()

let filters: WatchlistFilters = {}

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
  filters = {}
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
        <Watchlists
          data={data}
          filters={filters}
          filter={(next) => {
            filters = next
            refreshWork = render()
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

function field(label: string): HTMLInputElement | HTMLSelectElement {
  const matched = Array.from(container.querySelectorAll("label")).find(
    (item) => item.textContent === label
  )

  const input = document.getElementById(matched?.htmlFor ?? "")

  if (
    !(input instanceof HTMLInputElement) &&
    !(input instanceof HTMLSelectElement)
  )
    throw new Error(`Missing field ${label}`)

  return input
}

async function type(label: string, value: string): Promise<void> {
  const input = field(label)
  await act(async () => {
    const prototype =
      input instanceof HTMLSelectElement
        ? HTMLSelectElement.prototype
        : HTMLInputElement.prototype

    Object.getOwnPropertyDescriptor(prototype, "value")?.set?.call(input, value)
    input.dispatchEvent(
      new Event(input instanceof HTMLSelectElement ? "change" : "input", {
        bubbles: true,
      })
    )
  })
}

async function click(
  name: string,
  scope: ParentNode = document
): Promise<void> {
  const button = Array.from(scope.querySelectorAll("button")).find(
    (item) => (item.getAttribute("aria-label") ?? item.textContent) === name
  )

  if (!button) throw new Error(`Missing button ${name}`)
  await act(async () => {
    button.click()
    await work
    await refreshWork
  })
}

async function add(owner: string, title: string): Promise<void> {
  await type(`Title for ${owner}`, title)

  const form = container.querySelector(
    `form[aria-label="Add plan for ${owner}"]`
  )

  if (!form) throw new Error("Expected owner form")
  await click("Add plan", form)
}

async function available() {
  const data = await readWatchlists()

  if (data.state !== "available")
    throw new Error("Expected authenticated Watchlist read")

  return data
}

test("renders all owners and persists movie/whole-series additions and confirmed owner-specific removals", async () => {
  await render()
  expect(container.textContent).toContain("Add a household member in Wishlist")
  await writeWishlist({ kind: "create-owner", name: "Member A" })
  await writeWishlist({ kind: "create-owner", name: "Member B" })
  await render()
  expect(container.querySelectorAll("form")).toHaveLength(2)
  await add("Member A", "A Quiet Orbit")
  await type("Type for Member B", "series")
  await add("Member B", "The Lighthouse Journey")
  await render()
  const data = await available()
  expect(data.entries).toHaveLength(2)

  const series = data.entries.find(
    (entry) => entry.title === "The Lighthouse Journey"
  )

  expect(series).toMatchObject({ kind: "series", status: "planned" })
  expect(container.textContent).toContain("Whole series")
  expect(container.querySelector("time")?.dateTime).toMatch(
    /^\d{4}-\d{2}-\d{2}$/
  )
  await click("Remove A Quiet Orbit from Member A’s watchlist")
  expect(document.querySelector('[role="alertdialog"]')?.textContent).toContain(
    "Member A’s watchlist"
  )
  await click("Keep plan")
  expect((await available()).entries).toHaveLength(2)
  await click("Remove A Quiet Orbit from Member A’s watchlist")
  await click("Remove permanently")
  await render()
  expect(container.textContent).not.toContain("A Quiet Orbit")
  expect((await available()).entries).toEqual([series])
})

test("failed additions retain drafts, refresh recovers auth, and unavailable reads cannot look empty", async () => {
  await writeWishlist({ kind: "create-owner", name: "Member A" })
  await render()
  vi.stubEnv("API_KEY", "synthetic-rejected-key")
  await add("Member A", "A Quiet Orbit")
  expect(container.textContent).toContain("OpenHome rejected the API key")
  expect(field("Title for Member A").value).toBe("A Quiet Orbit")
  vi.stubEnv("API_KEY", "synthetic-dashboard-api-key")
  await click("Refresh watchlists")
  await add("Member A", "A Quiet Orbit")
  vi.stubEnv("API_KEY", "synthetic-rejected-key")
  await click("Remove A Quiet Orbit from Member A’s watchlist")
  await click("Remove permanently")
  expect(document.querySelector('[role="alertdialog"]')?.textContent).toContain(
    "OpenHome rejected the API key"
  )
  await click("Keep plan")
  await render()
  expect(container.textContent).toContain("Watchlists unavailable")
  expect(container.textContent).not.toContain("No plans yet")
  expect(container.textContent).not.toContain("synthetic-rejected-key")
  vi.stubEnv("API_KEY", "synthetic-dashboard-api-key")
  await click("Refresh watchlists")
  expect(container.textContent).toContain("A Quiet Orbit")
  expect((await available()).entries).toHaveLength(1)
})

test("a dropped accepted addition retains owner, whole-series kind and request ID across remount and retries", async () => {
  await writeWishlist({ kind: "create-owner", name: "Member A" })
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
  await type("Type for Member A", "series")
  await add("Member A", "\u0085A Quiet Orbit\u0085")
  expect(container.textContent).toContain("The response was lost")
  await act(async () => root.unmount())
  root = createRoot(container)
  await render()
  expect(field("Title for Member A").value).toBe("A Quiet Orbit")
  expect(field("Type for Member A").value).toBe("series")
  await add("Member A", "A Quiet Orbit")
  expect(requests).toHaveLength(2)
  expect(requests[0]).toBe(requests[1])
  expect((await available()).entries).toHaveLength(1)
  expect(container.textContent).toContain("Plan saved to OpenHome")

  // A second intention loses its response, then another client removes the accepted entry.
  drop = true
  await add("Member A", "The Lighthouse Journey")
  expect(container.textContent).toContain("The response was lost")

  const removed = (await available()).entries.find(
    (entry) => entry.title === "The Lighthouse Journey"
  )

  if (removed === undefined)
    throw new Error("Expected accepted plan before remote removal")

  await writeWatchlist({
    action: "remove",
    owner_id: removed.owner_id,
    entry_id: removed.id,
  })
  await act(async () => root.unmount())
  root = createRoot(container)
  await render()
  await add("Member A", "The Lighthouse Journey")
  expect(container.textContent).toContain("its entry was removed")
  await click("Refresh watchlists")
  await add("Member A", "The Lighthouse Journey")
  expect(requests[2]).toBe(requests[3])
  expect(requests[2]).toBe(requests[4])
  expect((await available()).entries).toHaveLength(1)

  const form = container.querySelector(
    'form[aria-label="Add plan for Member A"]'
  )

  if (form === null) throw new Error("Expected retry form")

  await click("Discard draft", form)
  expect(field("Title for Member A").value).toBe("")
  expect(sessionStorage.length).toBe(0)
  await add("Member A", "The Lighthouse Journey")
  expect(requests[5]).not.toBe(requests[2])
  const plans = (await available()).entries

  expect(plans).toHaveLength(2)
  expect(
    plans.find((entry) => entry.title === "The Lighthouse Journey")
  ).toMatchObject({ kind: "series" })
  expect(container.textContent).toContain("Plan saved to OpenHome")
})

test("status/date editing persists through remount and filters retain household ownership", async () => {
  await writeWishlist({ kind: "create-owner", name: "Member A" })
  await writeWishlist({ kind: "create-owner", name: "Member B" })
  await render()
  await type("Type for Member A", "series")
  await add("Member A", "A Quiet Orbit")
  await add("Member B", "The Lighthouse Journey")

  const original = (await available()).entries.find(
    (entry) => entry.title === "A Quiet Orbit"
  )

  if (!original) throw new Error("Expected persisted series")

  async function edit(): Promise<void> {
    const summary = container
      .querySelector<HTMLDetailsElement>(`details`)
      ?.querySelector("summary")

    if (!summary) throw new Error("Expected status/date editor")
    await act(async () => {
      summary.click()
      await new Promise((resolve) => setTimeout(resolve, 0))
    })
  }

  await edit()
  await type("Status for A Quiet Orbit / Member A", "watching")
  await click("Save status & dates")
  expect(container.textContent).toContain("Whole series")
  expect(
    (await available()).entries.find((entry) => entry.id === original.id)
  ).toMatchObject({
    status: "watching",
    date_added: original.date_added,
    completion_date: null,
  })
  await edit()
  await type("Status for A Quiet Orbit / Member A", "completed")
  await type("Date added for A Quiet Orbit / Member A", "2024-02-28")
  await type("Completion date for A Quiet Orbit / Member A", "2024-02-29")
  vi.stubEnv("API_KEY", "synthetic-rejected-key")
  await click("Save status & dates")
  expect(container.querySelector("details")?.open).toBe(true)
  expect(field("Completion date for A Quiet Orbit / Member A").value).toBe(
    "2024-02-29"
  )
  expect(container.querySelector("details")?.textContent).toContain(
    "OpenHome rejected the API key"
  )
  vi.stubEnv("API_KEY", "synthetic-dashboard-api-key")
  await click("Save status & dates")
  await act(async () => root.unmount())
  root = createRoot(container)
  await render()
  expect(container.textContent).toContain("Completed 2024-02-29")
  expect(
    (await available()).entries.find((entry) => entry.id === original.id)
  ).toMatchObject({
    status: "completed",
    kind: "series",
    date_added: "2024-02-28",
    completion_date: "2024-02-29",
  })
  await type("Household member", original.owner_id)
  await act(async () => {
    await refreshWork
  })
  await type("Watch status", "planned")
  await act(async () => {
    await refreshWork
  })
  expect(container.querySelectorAll('[role="group"]')).toHaveLength(1)
  expect(container.textContent).toContain("No planned entries")
  expect(container.textContent).not.toContain("The Lighthouse Journey")
  await type("Watch status", "completed")
  await act(async () => {
    await refreshWork
  })
  expect(container.textContent).toContain("A Quiet Orbit")
  await click("Refresh watchlists")
  expect(field("Watch status").value).toBe("completed")
  await edit()
  await type("Status for A Quiet Orbit / Member A", "planned")
  await click("Save status & dates")
  expect(container.textContent).toContain("No completed entries")
  await click("Clear filters")
  expect(container.querySelectorAll('[role="group"]')).toHaveLength(2)
  expect(
    (await available()).entries.find((entry) => entry.id === original.id)
  ).toMatchObject({
    status: "planned",
    date_added: "2024-02-28",
    completion_date: null,
  })
})
