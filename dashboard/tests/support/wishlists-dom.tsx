import { act } from "react"
import {
  createMemoryHistory,
  createRootRoute,
  createRouter,
  RouterContextProvider,
} from "@tanstack/react-router"
import { createRoot, type Root } from "react-dom/client"
import { afterAll, afterEach, beforeAll, beforeEach, expect, vi } from "vitest"
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
import { startAxumFixture } from "./axum-fixture"
import { startLossyAxum } from "./lost-response"
import { button, settle, sheet } from "./lists-dom"

let fixture: Awaited<ReturnType<typeof startAxumFixture>>

let apiUrl: string

let root: Root

export let container: HTMLDivElement

const proxies: Array<() => Promise<void>> = []

export let mutationWork: Promise<WishlistMutation> = Promise.resolve({
  state: "saved",
})

export let refreshWork: Promise<void> = Promise.resolve()

export let metadataWork: Promise<unknown> = Promise.resolve()

type Harness = {
  search: WishSearch
  metadata: WishlistMetadataLoader
  refreshFails: boolean
  held: Promise<void> | undefined
}

export const harness: Harness = {
  search: {},
  metadata: previewWishlistUrl,
  refreshFails: false,
  held: undefined,
}

export let sent: WishlistCommand[] = []

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
  harness.search = {}
  harness.metadata = previewWishlistUrl
  harness.refreshFails = false
  sent = []
  harness.held = undefined
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

export async function render(): Promise<void> {
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
          search={harness.search}
          choose={(next) => {
            harness.search = next
            refreshWork = render()
          }}
          change={(command) => {
            sent.push(command)
            mutationWork =
              harness.held === undefined
                ? writeWishlist(command)
                : harness.held.then(() => writeWishlist(command))

            return mutationWork
          }}
          refresh={() => {
            if (harness.refreshFails)
              return Promise.reject(new Error("Synthetic outage"))

            refreshWork = render()

            return refreshWork
          }}
          metadata={(request) => {
            const work = harness.metadata(request)
            metadataWork = work

            return work
          }}
        />
      </RouterContextProvider>
    )
  })
}

export async function press(element: HTMLButtonElement): Promise<void> {
  await act(async () => {
    element.click()
    await mutationWork
    await refreshWork
    await metadataWork
  })
  await settle()
}

export async function click(
  name: string | RegExp,
  scope: ParentNode = document
): Promise<void> {
  await press(button(name, scope))
}

export async function debounced(): Promise<void> {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 500))
    await metadataWork
  })
  await settle()
}

export async function remount(): Promise<void> {
  await act(async () => root.unmount())
  root = createRoot(container)
}

export function valued(
  value: string,
  scope: ParentNode = sheet()
): HTMLInputElement {
  const input = Array.from(scope.querySelectorAll("input")).find(
    (element) => element.value === value
  )

  if (input === undefined) throw new Error(`Missing input ${value}`)

  return input
}

export function section(
  label: string,
  scope: ParentNode = sheet()
): HTMLElement {
  const element = scope.querySelector<HTMLElement>(
    `section[aria-label="${label}"]`
  )

  if (element === null) throw new Error(`Missing section ${label}`)

  return element
}

export async function plate(title: string): Promise<void> {
  const match = Array.from(
    container.querySelectorAll<HTMLButtonElement>('[data-slot="wish-plate"]')
  ).find((element) => element.textContent?.includes(title))

  if (match === undefined) throw new Error(`Missing plate ${title}`)
  await press(match)
}

export function titles(scope: ParentNode = container): string[] {
  return Array.from(
    scope.querySelectorAll('[data-slot="wish-title"]'),
    (element) => element.textContent ?? ""
  )
}

export async function available(): Promise<
  Extract<WishlistState, { state: "available" }>
> {
  const data = await readWishlists()

  if (data.state !== "available")
    throw new Error("Expected authenticated Wishlist read")

  return data
}

export async function household(people: Record<string, string[]>) {
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

export async function loseNextAddition(): Promise<void> {
  const proxy = await startLossyAxum(
    apiUrl,
    (request) => request.url === "/api/wishlist/items"
  )

  proxies.push(proxy.stop)
  vi.stubEnv("OPENHOME_API_URL", proxy.url)
}

export function withoutSessionStorage(): void {
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

export async function wish(
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

export function additions() {
  return sent.flatMap((command) =>
    command.kind === "add-item" ? [command] : []
  )
}
