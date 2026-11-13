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
import { CurrencyProvider, CurrencySwitch } from "@/components/currency"
import { PriceHistory } from "@/components/price-history"
import { currencyCookieName } from "@/lib/currency"
import { readCurrency } from "@/lib/currency.functions"
import { readBinder, readCardPrices } from "@/lib/binder.server"
import type { PricePoint, PricesState } from "@/lib/card-prices"
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

const point = (observed_on: string, usd: number): PricePoint => ({
  observed_on,
  usd,
  eur: usd / 2,
  dkk: usd * 4,
  rate_date: observed_on,
})

/** Four days of observations with two days missing in the middle. */
const withGap: PricesState = {
  state: "read",
  points: [
    point("2026-11-01", 2),
    point("2026-11-02", 2.5),
    point("2026-11-05", 3),
    point("2026-11-06", 3.5),
  ],
}

async function render(
  load: (card: string) => Promise<PricesState>
): Promise<void> {
  await act(async () =>
    root.render(
      <CurrencyProvider initial={readCurrency()}>
        <CurrencySwitch />
        <PriceHistory card="OP99-001" load={load} />
      </CurrencyProvider>
    )
  )
  await settle()
}

/** Intl puts a no-break space between a currency code and its amount. */
const plain = (text: string | null | undefined): string =>
  (text ?? "").replaceAll("\u00a0", " ")

const rows = (): string[][] =>
  Array.from(container.querySelectorAll("tbody tr")).map((row) =>
    Array.from(row.children).map((cell) => plain(cell.textContent))
  )

test("a day without an observation is a gap in the line, never a flat stretch", async () => {
  await render(() => Promise.resolve(withGap))

  expect(container.querySelector("[data-chart='card-price']")).not.toBeNull()
  expect(container.querySelectorAll("path.trend-line")).toHaveLength(2)
  expect(container.querySelector(".trend-point")).toBeNull()
  expect(rows()).toEqual([
    ["1 Nov 2026", "DKK 8.00"],
    ["2 Nov 2026", "DKK 10.00"],
    ["3 Nov 2026", "No observation"],
    ["4 Nov 2026", "No observation"],
    ["5 Nov 2026", "DKK 12.00"],
    ["6 Nov 2026", "DKK 14.00"],
  ])
  expect(plain(container.querySelector(".trend-summary")?.textContent)).toBe(
    "DKK 14.00 on 6 Nov 2026; 4 observations since 1 Nov 2026, 2 days without an observation."
  )
})

test("the line follows the chosen currency", async () => {
  await render(() => Promise.resolve(withGap))
  await act(async () => button("EUR", container).click())

  expect(rows()[0]).toEqual(["1 Nov 2026", "€1.00"])

  await act(async () => button("USD", container).click())

  expect(rows()[5]).toEqual(["6 Nov 2026", "US$3.50"])
})

test("a point with no rate yet is a gap in that currency, named as a missing rate", async () => {
  await render(() =>
    Promise.resolve({
      state: "read",
      points: [
        { ...point("2026-11-01", 2), dkk: null, eur: null, rate_date: null },
        point("2026-11-02", 2.5),
      ],
    })
  )

  expect(rows()).toEqual([
    ["1 Nov 2026", "No exchange rate"],
    ["2 Nov 2026", "DKK 10.00"],
  ])
})

test("an unreadable history says so and an empty one says none was recorded, with no chart", async () => {
  await render(() => Promise.resolve({ state: "unavailable" }))
  expect(container.textContent).toContain("Price history is unavailable")
  expect(container.querySelector("figure")).toBeNull()

  await render(() => Promise.resolve({ state: "read", points: [] }))
  expect(container.textContent).toContain("No price has been recorded")
  expect(container.querySelector("figure")).toBeNull()
})

async function stock(): Promise<void> {
  await writeWishlist({ kind: "create-owner", name: "Alex" })
  const data = await readBinder()
  const alex = data.state === "available" ? data.owners[0] : undefined

  if (alex === undefined) throw new Error("Missing owner")

  for (const [index, addition] of [
    { card: "OP99-001", quantity: 2 },
    { card: "OP99-001", language: "ja" },
  ].entries()) {
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

async function openCard(name: RegExp): Promise<HTMLElement> {
  await act(async () => button(name, container).click())
  await settle()

  return sheet()
}

test("an English card sheet charts the history Axum stored, and a Japanese one has none", async () => {
  await stock()

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
            prices={(card) => readCardPrices({ card })}
          />
        </CurrencyProvider>
      </RouterContextProvider>
    )
  )

  const english = await openCard(/^Captain Marlow \(001\), OP99-001, 2 copies/)
  const stored = await readCardPrices({ card: "OP99-001" })

  expect(stored.state === "read" && stored.points.length).toBe(1)
  expect(english.querySelector("[data-chart='card-price']")).not.toBeNull()
  expect(plain(english.querySelector(".trend-summary")?.textContent)).toMatch(
    /^DKK [\d.,]+ on \d+ \w+ \d{4}; 1 observation since /
  )
  expect(english.querySelector(".trend-point")).not.toBeNull()
  await act(async () => button("Close", document).click())

  const japanese = await openCard(/^Captain Marlow \(001\), OP99-001, Japanese/)

  expect(nameOf(japanese)).toContain("English printings only")
  expect(japanese.querySelector("[data-chart]")).toBeNull()
})
