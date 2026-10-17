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
import { DateBoard } from "@/components/date-board"
import { IdleMilestone } from "@/components/idle-milestone"
import {
  datesFor,
  leadFigure,
  type ImportantDate,
  type ImportantDateMutation,
} from "@/lib/important-dates"
import {
  readImportantDates,
  readIdleDates,
  writeImportantDate,
} from "@/lib/important-dates.server"
import type { PersonSearch } from "@/lib/lists"
import { writeWishlist } from "@/lib/wishlist.server"
import { startAxumFixture } from "./support/axum-fixture"
import { button, field, settle, sheet, type } from "./support/lists-dom"

let fixture: Awaited<ReturnType<typeof startAxumFixture>>

let root: Root

let container: HTMLDivElement

let work: Promise<ImportantDateMutation> = Promise.resolve({ state: "saved" })

let refreshWork: Promise<void> = Promise.resolve()

let search: PersonSearch = {}

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
})

afterEach(async () => {
  await act(async () => root.unmount())
  container.remove()

  const data = await available()

  for (const date of data.dates)
    await writeImportantDate({ action: "remove", date_id: date.id })

  for (const owner of data.owners)
    await writeWishlist({ kind: "remove-owner", owner_id: owner.id })
  vi.unstubAllEnvs()
  vi.unstubAllGlobals()
})

async function available() {
  const data = await readImportantDates()

  if (data.state !== "available") throw new Error("Expected a dates read")

  return data
}

async function render(): Promise<void> {
  const data = await readImportantDates()

  const router = createRouter({
    routeTree: createRootRoute(),
    history: createMemoryHistory(),
    isServer: false,
  })

  await act(async () =>
    root.render(
      <RouterContextProvider router={router}>
        <DateBoard
          data={data}
          search={search}
          choose={(next) => {
            search = next
            refreshWork = render()
          }}
          change={(command) => {
            work = writeImportantDate(command)

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

async function click(
  name: string | RegExp,
  scope: ParentNode = document
): Promise<void> {
  await act(async () => {
    button(name, scope).click()
    await work
    await refreshWork
  })
  await settle()
}

function titles(): string[] {
  return Array.from(
    container.querySelectorAll(".date-plate-title"),
    (title) => title.textContent ?? ""
  )
}

test("lead figures read years, months and days for since and a countdown for yearly", () => {
  const base: ImportantDate = {
    id: "00000000-0000-4000-8000-000000000000",
    title: "Together",
    date: "2024-02-14",
    kind: "since",
    person_ids: [],
    show_when_idle: false,
    milestone: null,
    elapsed: { years: 2, months: 3, days: 12, total_days: 832 },
    next: { date: "2027-02-14", days_until: 264, years: 3 },
  }

  expect(leadFigure(base)).toBe("2 years · 3 months · 12 days")
  // Idle's elapsed counter stays independent of the separately rendered milestone lead.
  expect(
    leadFigure({ ...base, milestone: { kind: "day_count", days: 1000 } })
  ).toBe("2 years · 3 months · 12 days")
  expect(
    leadFigure({
      ...base,
      elapsed: { years: 1, months: 0, days: 0, total_days: 365 },
    })
  ).toBe("1 year")
  expect(
    leadFigure({
      ...base,
      elapsed: { years: 0, months: 0, days: 0, total_days: 0 },
    })
  ).toBe("0 days")
  expect(leadFigure({ ...base, elapsed: null })).toBe("in 264 days")
  expect(leadFigure({ ...base, kind: "yearly" })).toBe("in 264 days")
  expect(
    leadFigure({
      ...base,
      kind: "yearly",
      next: { ...base.next, days_until: 1 },
    })
  ).toBe("Tomorrow")
  expect(
    leadFigure({
      ...base,
      kind: "yearly",
      next: { ...base.next, days_until: 0 },
    })
  ).toBe("Today")
  expect(
    datesFor(
      [
        { ...base, id: "a", person_ids: ["p1"] },
        { ...base, id: "b", person_ids: ["p2"] },
        { ...base, id: "c" },
      ],
      "p1"
    ).map((date) => date.id)
  ).toEqual(["a", "c"])
})

test("idle milestone leads with the highest priority visible date and rests when none qualify", async () => {
  const base: ImportantDate = {
    id: "00000000-0000-4000-8000-000000000000",
    title: "Moved in",
    date: "2024-01-01",
    kind: "since",
    person_ids: [],
    show_when_idle: true,
    elapsed: { years: 2, months: 8, days: 26, total_days: 1000 },
    next: { date: "2027-01-01", days_until: 96, years: 3 },
    milestone: { kind: "day_count", days: 1000 },
  }

  await act(async () =>
    root.render(
      <IdleMilestone
        dates={[
          {
            ...base,
            title: "Private",
            show_when_idle: false,
            milestone: { kind: "anniversary", years: 8 },
          },
          base,
        ]}
      />
    )
  )
  expect(container.textContent).toContain("1,000 days today")
  expect(container.textContent).toContain("Moved in")
  expect(container.textContent).not.toContain("Private")
  await act(async () =>
    root.render(
      <IdleMilestone
        dates={[
          base,
          {
            ...base,
            title: "Together",
            milestone: { kind: "anniversary", years: 2 },
          },
        ]}
      />
    )
  )
  expect(container.textContent).toContain("2 years today")
  expect(container.textContent).toContain("Together")
  expect(container.textContent).not.toContain("Moved in")
  await act(async () =>
    root.render(
      <IdleMilestone
        dates={[
          {
            ...base,
            title: "Sam's birthday",
            kind: "yearly",
            date: "1992-09-27",
            milestone: { kind: "yearly", years: 34 },
          },
        ]}
      />
    )
  )
  expect(container.textContent).toContain("Sam's birthday today")
  expect(container.textContent).toContain("34 years since 27 Sept 1992")
  await act(async () =>
    root.render(<IdleMilestone dates={[{ ...base, milestone: null }]} />)
  )
  expect(container.textContent).toBe("")
})

test("adds a tagged date, filters by person, edits it, switches idle and deletes after confirmation", async () => {
  await writeWishlist({ kind: "create-owner", name: "Member A" })
  await writeWishlist({ kind: "create-owner", name: "Member B" })

  const [memberA, memberB] = (await available()).owners

  if (memberA === undefined || memberB === undefined)
    throw new Error("Expected owners")

  await writeImportantDate({
    action: "add",
    title: "A friend's birthday",
    date: "2000-02-29",
    kind: "yearly",
    person_ids: [],
    show_when_idle: false,
    request_id: crypto.randomUUID(),
  })
  await writeImportantDate({
    action: "add",
    title: "Member B's birthday",
    date: "1990-07-09",
    kind: "yearly",
    person_ids: [memberB.id],
    show_when_idle: false,
    request_id: crypto.randomUUID(),
  })
  await render()

  await click("Add a date", container)
  await type(field("Title", sheet()), "  Together ")
  await type(field("Date", sheet()), "2019-02-14")
  await click("Member A", sheet())
  await click("Member B", sheet())
  await click("Add date", sheet())

  const together = (await available()).dates.find(
    (date) => date.title === "Together"
  )

  expect(together).toMatchObject({
    kind: "since",
    date: "2019-02-14",
    show_when_idle: false,
  })
  expect(together?.person_ids).toHaveLength(2)
  // Since dates lead, with Axum's figure on the plate.
  expect(titles()[0]).toBe("Together")
  expect(container.querySelector(".date-figure")?.textContent).toMatch(
    /\d+ years/
  )
  expect(container.textContent).toContain("14 Feb 2019")

  search = { person: memberA.id }
  await render()
  expect(titles()).toEqual(["Together", "A friend's birthday"])

  await click(/Together/, container.querySelector(".date-plates") ?? container)
  await click("Edit", sheet())
  await type(field("Title", sheet()), "Together again")
  await click("Yearly", sheet())
  await click("Member B", sheet())
  await click("Save date", sheet())

  const edited = (await available()).dates.find(
    (date) => date.id === together?.id
  )

  expect(edited).toMatchObject({
    title: "Together again",
    kind: "yearly",
    person_ids: [memberA.id],
  })

  const idle = sheet().querySelector<HTMLButtonElement>('[role="switch"]')

  expect(idle?.getAttribute("aria-checked")).toBe("false")
  await click(/^Show when idle/, sheet())
  expect(
    (await available()).dates.find((date) => date.id === together?.id)
      ?.show_when_idle
  ).toBe(true)

  await click("Delete", sheet())
  expect(
    sheet().querySelector('[aria-label="Delete date"]')?.textContent
  ).toContain("Together again")
  await click("Keep it", sheet())
  expect((await available()).dates).toHaveLength(3)
  await click("Delete", sheet())
  await click("Delete for good", sheet())
  expect((await available()).dates).toHaveLength(2)
  expect(titles()).toEqual(["A friend's birthday"])
})

test("a lost read says so instead of showing an empty list", async () => {
  vi.stubEnv("API_KEY", "wrong-key")
  await render()
  expect(container.textContent).toContain("Dates unavailable")
  expect(container.textContent).toContain("OpenHome rejected the API key")
  vi.stubEnv("API_KEY", "synthetic-dashboard-api-key")
})

test("the shared idle read returns only dates opted into room visibility", async () => {
  for (const [title, visible] of [
    ["Together", true],
    ["Private date", false],
  ] as const)
    await writeImportantDate({
      action: "add",
      title,
      date: "2019-02-14",
      kind: "since",
      person_ids: [],
      show_when_idle: visible,
      request_id: crypto.randomUUID(),
    })

  const read = await readIdleDates()
  expect(read.state).toBe("available")

  if (read.state !== "available") throw new Error("Expected the filtered read")
  expect(read.dates.map((date) => date.title)).toEqual(["Together"])
})
