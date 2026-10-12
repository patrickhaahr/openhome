// @vitest-environment happy-dom
// @vitest-environment-options {"settings":{"fetch":{"disableSameOriginPolicy":true}}}
// This harness also executes the private server transport; CORS applies to browser RPCs, not Axum calls.

import { spawn, type ChildProcess } from "node:child_process"
import { once } from "node:events"
import {
  createServer,
  type Server,
  type IncomingMessage,
  type ServerResponse,
} from "node:http"
import { resolve } from "node:path"
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
import { readWishlists, writeWishlist } from "@/lib/wishlist.server"
import type { WishlistState, WishlistMutation } from "@/lib/wishlist"

let api: ChildProcess | undefined

let apiUrl: string

let root: Root

let container: HTMLDivElement

const servers: Server[] = []

let mutationWork: Promise<WishlistMutation> = Promise.resolve({
  state: "saved",
})

let refreshWork: Promise<void> = Promise.resolve()

beforeAll(async () => {
  api = spawn(resolve("../api/target/debug/examples/dashboard_health"), [], {
    stdio: ["ignore", "pipe", "pipe"],
  })
  const child = api
  apiUrl = await new Promise<string>((resolve, reject) => {
    const timeout = setTimeout(
      () => reject(new Error("Axum startup timed out")),
      20000
    )

    child.stdout?.on("data", (chunk: Buffer) => {
      const url = chunk.toString().match(/http:\/\/127\.0\.0\.1:\d+/)?.[0]

      if (url !== undefined) {
        clearTimeout(timeout)
        resolve(url)
      }
    })
    child.once("error", (error) => {
      clearTimeout(timeout)
      reject(error)
    })
    child.once("exit", (code) => {
      clearTimeout(timeout)
      reject(new Error(`Axum exited: ${code}`))
    })
  })
})

afterAll(async () => {
  if (api !== undefined && api.exitCode === null && api.signalCode === null) {
    const exited = once(api, "exit")

    api.kill("SIGTERM")
    await exited
  }
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

async function render(): Promise<void> {
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
  })
}

function field(label: string): HTMLInputElement {
  const fieldLabel = Array.from(container.querySelectorAll("label")).find(
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
  { title: " A book ", retry: "A book", expected: ["A book"] },
  { title: "\u0085A book\u0085", retry: "A book", expected: ["A book"] },
  {
    title: "\uFEFFA book\uFEFF",
    retry: "A book",
    expected: ["A book", "\uFEFFA book\uFEFF"],
  },
])(
  "a lost addition respects API title identity after remount ($title), and auth failures retain the draft",
  async ({ title, retry, expected }) => {
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
    await submit("Something you’d like", title)
    expect(container.textContent).toContain("The response was lost")
    await act(async () => root.unmount())
    root = createRoot(container)
    await render()
    await submit("Something you’d like", retry)
    expect((await available()).items.map((item) => item.title).sort()).toEqual(
      expected
    )
    expect(container.textContent).toContain("Saved to OpenHome")
    vi.stubEnv("API_KEY", "synthetic-rejected-key")
    await submit("Something you’d like", "A rain hat")
    expect(container.textContent).toContain("OpenHome rejected the API key")
    expect(container.textContent).not.toContain("synthetic-rejected-key")
    expect(
      container.querySelector<HTMLInputElement>('input[value="A rain hat"]')
        ?.value
    ).toBe("A rain hat")
    vi.stubEnv("API_KEY", "synthetic-dashboard-api-key")
    await click("Refresh lists")
    await submit("Something you’d like", "A rain hat")
    expect((await available()).items.map((item) => item.title).sort()).toEqual(
      [...expected, "A rain hat"].sort()
    )
  }
)
