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
import { CurrencyProvider } from "@/components/currency"
import { currencyCookieName, formatMoney } from "@/lib/currency"
import { readCurrency } from "@/lib/currency.functions"
import { readBinder } from "@/lib/binder.server"
import { readWatchlists } from "@/lib/watchlist.server"
import { writeWishlist } from "@/lib/wishlist.server"
import { startAxumFixture } from "./support/axum-fixture"
import { button, nameOf, settle, sheet } from "./support/lists-dom"

let fixture: Awaited<ReturnType<typeof startAxumFixture>>

let root: Root

let container: HTMLDivElement

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
  // happy-dom keeps cookies across tests; expire the one under test.
  document.cookie = `${currencyCookieName}=; Path=/; Max-Age=0`
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

/** A page load: the server reads the cookie, the Binder renders from it. */
async function load(): Promise<void> {
  const data = await readBinder()

  const router = createRouter({
    routeTree: createRootRoute(),
    history: createMemoryHistory(),
    isServer: false,
  })

  await act(async () =>
    root.render(
      <RouterContextProvider router={router}>
        <CurrencyProvider initial={readCurrency()}>
          <BinderBook
            data={data}
            search={{}}
            choose={() => undefined}
            change={() => Promise.resolve({ state: "saved" })}
            refresh={() => Promise.resolve()}
          />
        </CurrencyProvider>
      </RouterContextProvider>
    )
  )
}

/** Alex's Binder: two English cards, one stale, and two entries with no Card Value. */
async function stock(): Promise<void> {
  await writeWishlist({ kind: "create-owner", name: "Alex" })
  const data = await readBinder()
  const alex = data.state === "available" ? data.owners[0] : undefined

  if (alex === undefined) throw new Error("Missing owner")

  const additions = [
    { card: "OP99-001", quantity: 2 },
    { card: "OP99-006" },
    { card: "OP99-002_p1" },
    { card: "OP99-001", language: "ja" },
  ]

  for (const [index, addition] of additions.entries()) {
    const response = await fetch(`${fixture.url}/api/binder/entries`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${key}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        owner_id: alex.id,
        card_id: addition.card,
        quantity: addition.quantity,
        language: addition.language,
        request_id: `${alex.id}-${index}`,
      }),
    })

    if (!response.ok) throw new Error(`Addition failed ${response.status}`)
  }
}

const headerFigure = (): string =>
  container.querySelector(".binder-value-bar .binder-value-figure")
    ?.textContent ?? ""

async function press(element: HTMLButtonElement): Promise<void> {
  await act(async () => element.click())
  await settle()
}

async function openCard(name: RegExp): Promise<HTMLElement> {
  await press(button(name, container))

  return sheet()
}

test("the header shows the Binder Value of the priced entries, what it leaves out, and the oldest price", async () => {
  await stock()
  await load()

  const data = await readBinder()
  const value = data.state === "available" ? data.values[0] : undefined

  if (value === undefined) throw new Error("Missing Binder Value")

  expect(headerFigure()).toBe(formatMoney(value.dkk, "DKK"))
  expect(headerFigure()).toMatch(/^DKK\s/)
  expect(
    container.querySelector(".binder-value-bar .binder-value-note")?.textContent
  ).toMatch(/^2 unpriced\. Oldest price \d+ \w+( \d{4})?\.$/)
})

test("the currency switch is remembered in a cookie and a reload renders it from the first paint", async () => {
  await stock()
  await load()

  const data = await readBinder()
  const value = data.state === "available" ? data.values[0] : undefined

  if (value === undefined) throw new Error("Missing Binder Value")

  expect(button("DKK", container).getAttribute("aria-pressed")).toBe("true")

  await press(button("EUR", container))

  expect(headerFigure()).toBe(formatMoney(value.eur, "EUR"))
  expect(document.cookie).toContain(`${currencyCookieName}=EUR`)

  await act(async () => root.unmount())
  root = createRoot(container)
  await load()

  expect(button("EUR", container).getAttribute("aria-pressed")).toBe("true")
  expect(headerFigure()).toBe(formatMoney(value.eur, "EUR"))

  await press(button("USD", container))
  expect(headerFigure()).toBe(formatMoney(value.usd, "USD"))
})

test("a card sheet shows the Card Value in the chosen currency with its date, and a stale badge only when the price is old", async () => {
  await stock()
  await load()
  await press(button("USD", container))

  const fresh = await openCard(/^Captain Marlow \(001\), OP99-001, 2 copies/)

  expect(nameOf(fresh.querySelector(".binder-value") ?? fresh)).toMatch(
    /^Card Value\s*\$?US\$2\.25Observed /
  )
  expect(fresh.querySelector(".binder-stale")).toBeNull()
  await press(button("Close", document))

  const stale = await openCard(/^Cinder Tidemarker \(006\), OP99-006/)

  expect(stale.querySelector(".binder-value-figure")?.textContent).toBe(
    "US$2.80"
  )
  expect(stale.querySelector(".binder-stale")?.textContent).toBe("Stale")
})

test("a Japanese entry and a card the catalogue does not price say why they have no value", async () => {
  await stock()
  await load()

  const japanese = await openCard(/^Captain Marlow \(001\), OP99-001, Japanese/)

  expect(japanese.querySelector(".binder-value-figure")).toBeNull()
  expect(japanese.querySelector(".binder-value")?.textContent).toContain(
    "English printings only"
  )
  await press(button("Close", document))

  const unpriced = await openCard(/Parallel\), OP99-002/)

  expect(unpriced.querySelector(".binder-value")?.textContent).toContain(
    "no price for this card"
  )
})
