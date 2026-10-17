// @vitest-environment happy-dom
import { act } from "react"
import { createRoot, type Root } from "react-dom/client"
import { afterEach, beforeEach, expect, test, vi } from "vitest"
import { IdleDates, type IdleDatesLead } from "@/components/idle-dates"
import { IdleMilestone } from "@/components/idle-milestone"
import { DWELL_MS } from "@/components/idle-screen"
import type { ImportantDate, ImportantDatesState } from "@/lib/important-dates"

const readDates = vi.fn<() => Promise<ImportantDatesState>>()

const together: ImportantDate = {
  id: "together",
  title: "Together",
  date: "2019-02-14",
  kind: "since",
  person_ids: [],
  show_when_idle: true,
  milestone: null,
  elapsed: { years: 7, months: 8, days: 3, total_days: 2802 },
  next: { date: "2027-02-14", days_until: 120, years: 8 },
}

function birthday(title: string, days: number): ImportantDate {
  return {
    ...together,
    id: title,
    title,
    kind: "yearly",
    next: { ...together.next, days_until: days },
  }
}

function available(
  dates: ImportantDate[]
): Extract<ImportantDatesState, { state: "available" }> {
  return { state: "available", owners: [], on: "2026-10-17", dates }
}

let root: Root

let container: HTMLDivElement

beforeEach(() => {
  vi.useFakeTimers()
  vi.setSystemTime(new Date("2026-10-17T12:00:00Z"))
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  container = document.createElement("div")
  document.body.append(container)
  root = createRoot(container)
  readDates.mockReset()
})

afterEach(async () => {
  await act(async () => root.unmount())
  container.remove()
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

async function render(lead?: IdleDatesLead): Promise<void> {
  await act(async () => root.render(<IdleDates lead={lead} read={readDates} />))
}

test("shows allowed elapsed figures and total days with a lead slot first", async () => {
  readDates.mockResolvedValue(
    available([
      together,
      {
        ...together,
        id: "private",
        title: "Private date",
        show_when_idle: false,
      },
    ])
  )
  await render(<p>Milestone lead</p>)
  expect(
    container.querySelector(".idle-dates")?.firstElementChild?.textContent
  ).toBe("Milestone lead")
  expect(container.textContent).toContain("Together")
  expect(container.textContent).toContain("7 years · 8 months · 3 days")
  expect(container.textContent).toContain("2,802 days")
  expect(container.textContent).not.toContain("Private date")
  await render((dates) => (
    <p>{dates.map((date) => date.title).join(", ")} lead</p>
  ))
  expect(container.querySelector(".idle-dates-lead")?.textContent).toBe(
    "Together lead"
  )
})

test.each(["empty", "unavailable", "rejected"])(
  "quietly adds no content for %s dates",
  async (state) => {
    if (state === "rejected") readDates.mockRejectedValue(new Error("Offline"))
    else
      readDates.mockResolvedValue(
        state === "empty"
          ? available([])
          : { state: "unavailable", reason: "unreachable" }
      )
    await render()
    expect(container.textContent).toBe("")
    expect(container.querySelector(".idle-dates")).toBeNull()
  }
)

test.each([false, true])(
  "rotates nearest yearly dates slowly with today and tomorrow phrasing (reduced motion: %s)",
  async (reduced) => {
    vi.stubGlobal("matchMedia", () => ({
      matches: reduced,
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
    }))
    readDates.mockResolvedValue(
      available([
        birthday("Alex's birthday", 18),
        birthday("First date", 1),
        birthday("Anniversary", 0),
      ])
    )
    await render()
    expect(container.textContent).toBe("Anniversary today")
    await act(async () => vi.advanceTimersByTimeAsync(DWELL_MS - 1))
    expect(container.textContent).toBe("Anniversary today")
    await act(async () => vi.advanceTimersByTimeAsync(1))
    expect(container.textContent).toBe("First date tomorrow")
    await act(async () => vi.advanceTimersByTimeAsync(DWELL_MS))
    expect(container.textContent).toBe("Alex's birthday in 18 days")
  }
)

test.each([
  ["2026-10-17T21:59:59Z", "2026-10-17"],
  ["2026-03-29T21:59:59Z", "2026-03-29"],
  ["2026-10-25T22:59:59Z", "2026-10-25"],
])(
  "refetches at Copenhagen midnight from %s without a reload",
  async (instant, on) => {
    vi.setSystemTime(new Date(instant))
    readDates
      .mockResolvedValueOnce({ ...available([together]), on })
      .mockResolvedValueOnce(
        available([
          {
            ...together,
            elapsed: { years: 7, months: 8, days: 4, total_days: 2803 },
          },
        ])
      )
    await render()
    expect(container.textContent).toContain("2,802 days")
    await act(async () => vi.advanceTimersByTimeAsync(999))
    expect(container.textContent).toContain("2,802 days")
    await act(async () => vi.advanceTimersByTimeAsync(1001))
    expect(container.textContent).toContain("2,803 days")
  }
)

test("shares the milestone lead and elapsed counter until Copenhagen midnight", async () => {
  vi.setSystemTime(new Date("2026-10-17T21:59:59Z"))

  const movedIn: ImportantDate = {
    ...together,
    title: "Moved in",
    elapsed: { years: 2, months: 8, days: 26, total_days: 1000 },
    milestone: { kind: "day_count", days: 1000 },
  }

  readDates
    .mockResolvedValueOnce(
      available([
        movedIn,
        {
          ...together,
          title: "Private anniversary",
          show_when_idle: false,
          milestone: { kind: "anniversary", years: 8 },
        },
      ])
    )
    .mockResolvedValueOnce(
      available([
        {
          ...movedIn,
          milestone: null,
          elapsed: { years: 2, months: 8, days: 27, total_days: 1001 },
        },
      ])
    )
  await render((dates) => <IdleMilestone dates={dates} />)
  expect(container.querySelector(".idle-milestone")?.textContent).toContain(
    "1,000 days today"
  )
  expect(container.querySelector(".idle-since-figure")?.textContent).toBe(
    "2 years · 8 months · 26 days"
  )
  expect(container.textContent).not.toContain("Private anniversary")
  await act(async () => vi.advanceTimersByTimeAsync(2000))
  expect(container.querySelector(".idle-milestone")).toBeNull()
  expect(container.querySelector(".idle-since-figure")?.textContent).toBe(
    "2 years · 8 months · 27 days"
  )
  expect(container.textContent).toContain("1,001 days")
})

test("leaves no date overlay when the milestone lead has no opted-in dates", async () => {
  readDates.mockResolvedValue(available([]))
  await render((dates) => <IdleMilestone dates={dates} />)
  expect(container.querySelector(".idle-dates")).toBeNull()
})
