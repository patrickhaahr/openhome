// @vitest-environment happy-dom
// @vitest-environment-options {"settings":{"fetch":{"disableSameOriginPolicy":true}}}

import { act } from "react"
import { expect, test, vi } from "vitest"
import { z } from "zod"
import { readWatchlists, writeWatchlist } from "@/lib/watchlist.server"
import { button, field, settle, sheet, type } from "./support/lists-dom"
import {
  additions,
  available,
  click,
  container,
  debounced,
  harness,
  household,
  loseNextAddition,
  mutationWork,
  plate,
  refreshWork,
  remount,
  render,
  section,
  titles,
  valued,
  wish,
  withoutSessionStorage,
} from "./support/wishlists-dom"

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
  expect(
    sheet().querySelector('[data-slot="list-failure"]')?.textContent
  ).toContain("This name or request is already in use")
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

  expect(titles(section("Member A", container))).toEqual([
    "A warm jacket",
    "A scarf",
    "A tent",
  ])
  expect(titles(section("Member B", container))).toEqual(["A book"])
  expect(section("Member A", container).textContent).toContain("Outdoors")
  expect(section("Member A", container).textContent).toContain("×2")

  await click(/^Member A/, container)
  expect(harness.search).toEqual({
    person: home.owner("Member A").id,
    list: undefined,
  })
  expect(titles()).not.toContain("A book")
  expect(titles(section("Outdoors", container))).toEqual(["A tent"])
  await click(
    /^Outdoors/,
    container.querySelector('[aria-label="Member A’s lists"]') ?? container
  )
  expect(harness.search.list).toBe(home.list("Member A", "Outdoors").id)
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
  await act(async () => {
    sheet()
      .querySelector("form")
      ?.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }))
    await mutationWork
    await refreshWork
  })
  expect(
    sheet().querySelector('[data-slot="list-failure"]')?.textContent
  ).toContain("Check the fields")
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

  harness.metadata = async (request) => {
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

  harness.search = { person: list.owner_id, list: list.id }
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
  expect(
    container.querySelector('[data-slot="wish-plate"] img')?.getAttribute("src")
  ).toMatch(
    new RegExp(
      `^/wish-image/${list.owner_id}/${list.id}/${saved?.id}\\?v=\\w+$`
    )
  )

  harness.metadata = async () => ({
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
  harness.metadata = () => Promise.reject(new Error("Synthetic outage"))
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
    harness.search = { person: list.owner_id, list: list.id }
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

    await remount()
    await render()
    await click("Add a wish", container)

    if (rich) {
      expect(field("Price", sheet()).value).toBe("DKK 199.95")
      expect(
        sheet().querySelector('[data-slot="stepper"] output')?.textContent
      ).toBe("2")
    }

    await type(field("Title", sheet()), retry)
    await click("Add wish", sheet())
    expect((await available()).items.map((item) => item.title).sort()).toEqual(
      expected
    )
    expect(document.querySelector('[data-slot="sheet"]')).toBeNull()

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

test("a saved wish stays acknowledged when the view cannot be re-read, and its retry intention is spent", async () => {
  const home = await household({ "Member A": ["Ideas"] })
  const list = home.list("Member A", "Ideas")

  harness.search = { person: list.owner_id, list: list.id }
  await render()
  await click("Add a wish", container)
  await type(field("Title", sheet()), "A book")
  harness.refreshFails = true
  await click("Add wish", sheet())

  expect((await available()).items.map((item) => item.title)).toEqual([
    "A book",
  ])
  expect(document.querySelector('[data-slot="sheet"]')).toBeNull()
  expect(
    container.querySelector('[data-slot="list-failure"]')?.textContent
  ).toContain("Saved to OpenHome, but the view could not update")
  expect(titles()).toEqual([])
  expect(sessionStorage.getItem(`openhome.pending-wish.${list.id}`)).toBeNull()

  harness.refreshFails = false
  await render()
  expect(titles()).toEqual(["A book"])
  expect(additions()).toHaveLength(1)
})

test("a rejected addition keeps its request ID and arguments for a retry after remount, and changed input gets a new one", async () => {
  const home = await household({ "Member A": ["Ideas"] })
  const list = home.list("Member A", "Ideas")

  harness.search = { person: list.owner_id, list: list.id }
  await render()
  await click("Add a wish", container)
  await type(field("Price", sheet()), "DKK 99")
  await type(field("Title", sheet()), "A book")
  vi.stubEnv("API_KEY", "synthetic-rejected-key")
  await click("Add wish", sheet())
  expect(sheet().textContent).toContain("OpenHome rejected the API key")

  await remount()
  vi.stubEnv("API_KEY", "synthetic-dashboard-api-key")
  await render()
  await click("Add a wish", container)
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
  harness.search = { person: list.owner_id, list: list.id }
  await render()
  await click("Add a wish", container)
  await type(field("Title", sheet()), "A book")
  await click("Add wish", sheet())
  expect(sheet().textContent).toContain("The response was lost")
  expect(field("Title", sheet()).value).toBe("A book")
  await click("Add wish", sheet())

  const [first, retry] = additions()

  expect(retry).toEqual(first)
  expect((await available()).items.map((item) => item.title)).toEqual([
    "A book",
  ])
  expect(document.querySelector('[data-slot="sheet"]')).toBeNull()
})

test("a closed sheet's late save does not close the sheet opened after it", async () => {
  const home = await household({ "Member A": ["Ideas"] })
  const list = home.list("Member A", "Ideas")
  let release: () => void = () => undefined

  harness.search = { person: list.owner_id, list: list.id }
  await render()
  await click("Add a wish", container)
  await type(field("Title", sheet()), "A book")
  harness.held = new Promise<void>((resolve) => {
    release = resolve
  })

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
  expect(sheet().textContent).toContain("Household and lists")
  expect(sessionStorage.getItem(`openhome.pending-wish.${list.id}`)).toBeNull()
})
