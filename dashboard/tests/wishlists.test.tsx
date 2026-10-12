// @vitest-environment happy-dom
// @vitest-environment-options {"settings":{"fetch":{"disableSameOriginPolicy":true}}}
// This harness also executes the private server transport; CORS applies to browser RPCs, not Axum calls.

import { once } from "node:events"
import {
  createServer,
  type Server,
  type IncomingMessage,
  type ServerResponse,
} from "node:http"
import { act } from "react"
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
import { Wishlists } from "@/components/wishlists"
import {
  readWishlists,
  writeWishlist,
  previewWishlistUrl,
} from "@/lib/wishlist.server"
import type {
  WishlistState,
  WishlistMutation,
  WishlistMetadataLoader,
} from "@/lib/wishlist"

import { startAxumFixture } from "./support/axum-fixture"
import { readWatchlists, writeWatchlist } from "@/lib/watchlist.server"

let fixture: Awaited<ReturnType<typeof startAxumFixture>>

let apiUrl: string

let root: Root

let container: HTMLDivElement

const servers: Server[] = []

let mutationWork: Promise<WishlistMutation> = Promise.resolve({
  state: "saved",
})

let refreshWork: Promise<void> = Promise.resolve()

let metadataWork: Promise<unknown> = Promise.resolve()

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

  for (const server of servers.splice(0)) {
    server.closeAllConnections()
    await new Promise<void>((resolve, reject) =>
      server.close((error) => (error ? reject(error) : resolve()))
    )
  }

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

async function render(
  metadata: WishlistMetadataLoader = previewWishlistUrl
): Promise<void> {
  const data = await readWishlists()

  await act(async () => {
    root.render(
      <Wishlists
        data={data}
        change={(command) => {
          mutationWork = writeWishlist(command)

          return mutationWork
        }}
        refresh={() => {
          refreshWork = render()

          return refreshWork
        }}
        metadata={(request) => {
          const work = metadata(request)
          metadataWork = work

          return work
        }}
      />
    )
  })
}

function button(name: string, scope: ParentNode = document): HTMLButtonElement {
  const match = Array.from(scope.querySelectorAll("button")).find(
    (element) =>
      (element.getAttribute("aria-label") ?? element.textContent) === name
  )

  if (match === undefined) throw new Error(`Missing button: ${name}`)

  return match
}

async function click(name: string): Promise<void> {
  await clickButton(button(name))
}

async function clickButton(element: HTMLButtonElement): Promise<void> {
  await act(async () => {
    element.click()
    await mutationWork
    await refreshWork
    await metadataWork
  })
}

function field(label: string): HTMLInputElement {
  const scope =
    container.querySelector('form[aria-label="Save wish"]') ?? container

  const fieldLabel = Array.from(scope.querySelectorAll("label")).find(
    (element) => element.textContent === label
  )

  const input = document.getElementById(fieldLabel?.htmlFor ?? "")

  if (!(input instanceof HTMLInputElement))
    throw new Error(`Missing field: ${label}`)

  return input
}

async function type(label: string, value: string): Promise<void> {
  const input = field(label)

  await act(async () => {
    // Native setter represents typing rather than replacing React's tracked value.
    Object.getOwnPropertyDescriptor(
      HTMLInputElement.prototype,
      "value"
    )?.set?.call(input, value)
    input.dispatchEvent(new Event("input", { bubbles: true }))
  })
}

async function submit(label: string, value: string): Promise<void> {
  await type(label, value)
  const input = field(label)

  const form = input.closest("form")
  const submitButton = form?.querySelector("button")

  if (submitButton === null || submitButton === undefined)
    throw new Error("Expected form submit")

  await clickButton(submitButton)
}

async function available(): Promise<
  Extract<WishlistState, { state: "available" }>
> {
  const data = await readWishlists()

  if (data.state !== "available")
    throw new Error("Expected authenticated Wishlist read")

  return data
}

test("renders owner-aware CRUD, failed edits and confirmation against migrated authenticated Axum", async () => {
  await render()
  expect(container.textContent).toContain("Add a household member")
  await submit("New household member", "Member A")
  await submit("New household member", "Member B")
  await submit("List name for Member A", "Ideas")
  await submit("List name for Member A", "Outdoors")
  await submit("List name for Member B", "Ideas")
  expect(container.textContent).toContain("Member A")
  expect(container.textContent).toContain("Member B")

  const memberLists = container.querySelector(
    'section[aria-label="Member A’s lists"]'
  )

  if (memberLists === null) throw new Error("Expected Member A’s lists")

  await clickButton(button("Ideas0 wishes", memberLists))
  await submit("Something you’d like", "A warm jacket")
  await click("Edit A warm jacket")
  await submit("Wish title", "A waterproof jacket")
  expect(container.textContent).toContain("A waterproof jacket")
  await click("Rename list")
  await submit("List name", "Outdoors")
  expect(container.textContent).toContain(
    "This name or request is already in use"
  )
  expect(
    container.querySelector<HTMLInputElement>('input[value="Outdoors"]')?.value
  ).toBe("Outdoors")
  await submit("List name", "Clothes")
  await click("Remove list")
  expect(document.querySelector('[role="alertdialog"]')?.textContent).toContain(
    "all 1 wish"
  )
  await click("Keep it")
  expect((await available()).items).toHaveLength(1)
  await click("Remove list")
  await click("Remove permanently")
  expect(container.textContent).not.toContain("A waterproof jacket")
  const data = await available()

  expect(data.items).toEqual([])
  expect(data.lists.map((list) => list.name).sort()).toEqual([
    "Ideas",
    "Outdoors",
  ])
  await submit("Name for Member A", "Member C")
  expect(container.textContent).toContain("Member C")
  expect(container.textContent).not.toContain("Member A")
  await render()
  expect(container.textContent).toContain("Member C")
  expect(container.textContent).toContain("Member B")

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
  await click("Remove Member C")
  expect(document.querySelector('[role="alertdialog"]')?.textContent).toContain(
    "wishlists, wishes and watchlist entries"
  )
  await click("Keep it")
  const retained = await readWatchlists()

  expect(
    retained.state === "available" ? retained.entries.length : undefined
  ).toBe(1)
  await click("Remove Member C")
  await click("Remove permanently")
  const remaining = await readWatchlists()

  expect(
    remaining.state === "available" ? remaining.entries : undefined
  ).toEqual([])
  expect((await available()).owners.map((owner) => owner.name)).toEqual([
    "Member B",
  ])
})

test("URL suggestions remain editable and only save on explicit addition", async () => {
  await render()
  await submit("New household member", "Member A")
  await submit("List name for Member A", "Ideas")
  const list = (await available()).lists[0]

  if (!list) throw new Error("Expected list")

  // A protocol fixture exercises UI review, not extraction (owned by authenticated Axum tests).
  await render(async (request) => {
    const { url, ...destination } = request

    expect(destination).toEqual({
      owner_id: list.owner_id,
      list_id: list.id,
    })
    expect(url).toMatch(/^https:\/\/example\.com\/(product|another-product)$/)

    return {
      state: "preview",
      data: {
        status: "ready",
        title: request.url.endsWith("another-product")
          ? "Suggested hat"
          : "Suggested coat",
        url: request.url,
        image_url: "https://example.com/coat.png",
        price: "DKK 199.95",
      },
    }
  })
  await type("Product URL", "https://example.com/product")
  await click("Read product details")
  expect(field("Something you’d like").value).toBe("Suggested coat")
  expect(field("Image URL").value).toBe("https://example.com/coat.png")
  expect(field("Price (include currency)").value).toBe("DKK 199.95")
  expect((await available()).items).toEqual([])
  await type("Product URL", "https://example.com/another-product")
  expect(field("Something you’d like").value).toBe("")
  expect(field("Image URL").value).toBe("")
  expect(field("Price (include currency)").value).toBe("")
  await click("Read product details")
  expect(field("Something you’d like").value).toBe("Suggested hat")
  await type("Product URL", "https://example.com/product")
  await click("Read product details")
  await type("Image URL", "https://example.com/reviewed.png")
  await type("Price (include currency)", "DKK 179.95")
  await submit("Something you’d like", "My coat")
  expect((await available()).items[0]).toMatchObject({
    title: "My coat",
    list_id: list.id,
    url: "https://example.com/product",
    image_url: "https://example.com/reviewed.png",
    price: "DKK 179.95",
  })
  await render(async () => ({
    state: "preview",
    data: {
      status: "partial",
      title: "Suggested title",
      url: "https://example.com/other",
      image_url: null,
      price: null,
    },
  }))
  await type("Something you’d like", "My own idea")
  await type("Product URL", "https://example.com/other")
  await click("Read product details")
  expect(field("Something you’d like").value).toBe("My own idea")
  await submit("Something you’d like", "My own idea")
  expect((await available()).items).toHaveLength(2)
})

test("rejected URL assistance keeps the manual draft and can still add a wish", async () => {
  await render()
  await submit("New household member", "Member A")
  await submit("List name for Member A", "Ideas")
  await type("Product URL", "http://127.0.0.1/product")
  await click("Read product details")
  expect(container.textContent).toContain("enter the details manually")
  expect(field("Product URL").value).toBe("http://127.0.0.1/product")
  expect((await available()).items).toEqual([])
  await type("Product URL", "https://example.com/product")
  await submit("Something you’d like", "A manually described coat")
  expect((await available()).items[0]).toMatchObject({
    title: "A manually described coat",
    url: "https://example.com/product",
  })
})

test("refresh updates untouched member names and preserves edited drafts", async () => {
  await writeWishlist({ kind: "create-owner", name: "Member A" })
  const member = (await available()).owners[0]

  if (member === undefined) throw new Error("Expected owner")

  await render()
  await writeWishlist({
    kind: "rename-owner",
    owner_id: member.id,
    name: "Member B",
  })
  await click("Refresh lists")
  expect(field("Name for Member B").value).toBe("Member B")
  await type("Name for Member B", "My draft")
  await writeWishlist({
    kind: "rename-owner",
    owner_id: member.id,
    name: "Member C",
  })
  await click("Refresh lists")
  expect(field("Name for Member C").value).toBe("My draft")
  await submit("Name for Member C", "My draft")
  expect((await available()).owners[0]?.name).toBe("My draft")
  await writeWishlist({
    kind: "rename-owner",
    owner_id: member.id,
    name: "Member D",
  })
  await click("Refresh lists")
  expect(field("Name for Member D").value).toBe("Member D")
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
    title: "\uFEFFA book\uFEFF",
    retry: "A book",
    expected: ["A book", "\uFEFFA book\uFEFF"],
    legacy: false,
    rich: false,
  },
])(
  "a lost addition respects API title identity after remount ($title), and auth failures retain the draft",
  async ({ title, retry, expected, legacy, rich }) => {
    await writeWishlist({ kind: "create-owner", name: "Member A" })
    const member = (await available()).owners[0]

    if (member === undefined) throw new Error("Expected owner")

    await writeWishlist({
      kind: "create-list",
      owner_id: member.id,
      name: "Ideas",
    })
    let loseAddition = true

    async function forward(
      request: IncomingMessage,
      response: ServerResponse
    ): Promise<void> {
      const body = await new Promise<string>((resolveBody) => {
        let text = ""
        request.on("data", (chunk: Buffer) => {
          text += chunk.toString()
        })
        request.on("end", () => resolveBody(text))
      })

      const upstream = await fetch(`${apiUrl}${request.url ?? ""}`, {
        method: request.method ?? "GET",
        headers: {
          Authorization: request.headers.authorization ?? "",
          "Content-Type": "application/json",
        },
        body: body === "" ? null : body,
      })

      if (request.url === "/api/wishlist/items" && loseAddition) {
        loseAddition = false
        await upstream.text()
        request.socket.destroy()

        return
      }

      response.writeHead(upstream.status, {
        "Content-Type": "application/json",
      })
      response.end(await upstream.text())
    }

    const proxy = createServer((request, response) => {
      void forward(request, response).catch(() => response.destroy())
    })

    servers.push(proxy)
    proxy.listen(0, "127.0.0.1")
    await once(proxy, "listening")
    const address = z.object({ port: z.number() }).parse(proxy.address())

    vi.stubEnv("OPENHOME_API_URL", `http://127.0.0.1:${address.port}`)
    await render()

    if (rich) {
      await type("Price (include currency)", "DKK 199.95")
      await type("Quantity", "2")
    }

    await submit("Something you’d like", title)
    expect(container.textContent).toContain("The response was lost")

    if (legacy) {
      const list = (await available()).lists[0]

      if (!list) throw new Error("Expected list")
      const key = `openhome.pending-wish.${list.id}`

      const prior = z
        .object({ ownerId: z.uuid(), listId: z.uuid(), requestId: z.uuid() })
        .parse(JSON.parse(sessionStorage.getItem(key) ?? "null"))

      sessionStorage.setItem(key, JSON.stringify({ ...prior, title }))
    }

    await act(async () => root.unmount())
    root = createRoot(container)
    await render()

    if (rich) {
      expect(field("Price (include currency)").value).toBe("DKK 199.95")
      expect(field("Quantity").value).toBe("2")
    }

    await submit("Something you’d like", retry)
    expect((await available()).items.map((item) => item.title).sort()).toEqual(
      expected
    )
    expect(container.textContent).toContain("Saved to OpenHome")
    vi.stubEnv("API_KEY", "synthetic-rejected-key")
    await submit("Something you’d like", "A rain hat")
    expect(container.textContent).toContain("OpenHome rejected the API key")
    expect(container.textContent).not.toContain("synthetic-rejected-key")
    expect(field("Something you’d like").value).toBe("A rain hat")
    vi.stubEnv("API_KEY", "synthetic-dashboard-api-key")
    await click("Refresh lists")
    await submit("Something you’d like", "A rain hat")
    expect((await available()).items.map((item) => item.title).sort()).toEqual(
      [...expected, "A rain hat"].sort()
    )
  }
)

test("rich edits persist fields, preserve failed drafts, and clear optional context", async () => {
  await render()
  await submit("New household member", "Member A")
  await submit("List name for Member A", "Ideas")
  await submit("Something you’d like", "A warm jacket")
  await click("Edit A warm jacket")
  await type("Product URL", "https://example.com/jacket")
  await type("Image URL", "https://example.com/jacket.png")
  await type("Price (include currency)", "DKK 199.95")
  await type("Quantity", "2")

  const notes = container.querySelector<HTMLTextAreaElement>(
    'form[aria-label="Save wish"] textarea[name="notes"]'
  )

  const priority = container.querySelector<HTMLSelectElement>(
    'form[aria-label="Save wish"] select[name="priority"]'
  )

  if (!notes || !priority) throw new Error("Expected rich edit controls")
  await act(async () => {
    Object.getOwnPropertyDescriptor(
      HTMLTextAreaElement.prototype,
      "value"
    )?.set?.call(notes, "For winter")
    notes.dispatchEvent(new Event("input", { bubbles: true }))
    priority.value = "high"
    priority.dispatchEvent(new Event("change", { bubbles: true }))
  })
  await submit("Wish title", "A waterproof jacket")
  const item = (await available()).items[0]
  expect(item).toMatchObject({
    title: "A waterproof jacket",
    url: "https://example.com/jacket",
    image_url: "https://example.com/jacket.png",
    notes: "For winter",
    price: "DKK 199.95",
    quantity: 2,
    priority: "high",
  })
  expect(container.textContent).toContain("high priority")
  expect(
    container.querySelector('a[href="https://example.com/jacket"]')?.textContent
  ).toContain("Product link")
  await render()
  await click("Edit A waterproof jacket")
  await type("Product URL", "javascript:alert(1)")
  // Dispatch at the form seam so Axum validation is exercised even when native URL validation would stop submission.
  await act(async () => {
    field("Wish title")
      .closest("form")
      ?.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }))
    await mutationWork
    await refreshWork
  })
  expect(container.textContent).toContain("Check the fields")
  expect(field("Product URL").value).toBe("javascript:alert(1)")
  expect((await available()).items[0]).toEqual(item)
  await type("Product URL", "")
  await type("Image URL", "")
  await type("Price (include currency)", "")
  await submit("Wish title", "A waterproof jacket")
  expect((await available()).items[0]).toMatchObject({
    url: null,
    image_url: null,
    price: null,
    notes: "For winter",
    quantity: 2,
    priority: "high",
  })
})

test("refresh merges untouched rich fields while preserving edited drafts and partial saves", async () => {
  await render()
  await submit("New household member", "Member A")
  await submit("List name for Member A", "Ideas")
  await submit("Something you’d like", "A jacket")
  const item = (await available()).items[0]
  const list = (await available()).lists[0]

  if (item === undefined || list === undefined) throw new Error("Expected wish")

  await click("Edit A jacket")
  await type("Wish title", "My draft jacket")
  await writeWishlist({
    kind: "edit-item",
    owner_id: list.owner_id,
    list_id: list.id,
    item_id: item.id,
    quantity: 3,
    priority: "high",
    notes: "Updated through API",
  })
  await click("Refresh lists")
  expect(field("Quantity").value).toBe("3")
  expect(field("Wish title").value).toBe("My draft jacket")
  expect(
    container.querySelector<HTMLSelectElement>(
      'form[aria-label="Save wish"] select[name="priority"]'
    )?.value
  ).toBe("high")
  expect(
    container.querySelector<HTMLTextAreaElement>(
      'form[aria-label="Save wish"] textarea[name="notes"]'
    )?.value
  ).toBe("Updated through API")
  // A concurrent change without another refresh also stays intact when only the title was edited.
  await writeWishlist({
    kind: "edit-item",
    owner_id: list.owner_id,
    list_id: list.id,
    item_id: item.id,
    quantity: 4,
  })
  await submit("Wish title", "My draft jacket")
  expect((await available()).items[0]).toMatchObject({
    title: "My draft jacket",
    quantity: 4,
    priority: "high",
    notes: "Updated through API",
  })
})
