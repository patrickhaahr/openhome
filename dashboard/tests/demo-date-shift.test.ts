import { expect, test } from "vitest"
import {
  isNormalForm,
  shiftJson,
  shiftText,
  wholeDaysBetween,
} from "@/lib/demo/date-shift"

test("a day text moves by whole days across month ends", () => {
  expect(shiftText("2026-11-28", 3)).toBe("2026-12-01")
  expect(shiftText("2026-03-01", -1)).toBe("2026-02-28")
})

test("a datetime keeps its Copenhagen wall clock when the clocks change in between", () => {
  expect(shiftText("2026-10-20T07:12:00+02:00", 10)).toBe(
    "2026-10-30T07:12:00+01:00"
  )
  expect(shiftText("2026-03-20T23:25:00+01:00", 10)).toBe(
    "2026-03-30T23:25:00+02:00"
  )
})

test("shifting forward and back restores a datetime, fractions included", () => {
  const original = "2026-10-20T23:25:41.250+02:00"

  expect(shiftText(shiftText(original, 17), -17)).toBe(original)
})

test("text that is not a date passes through", () => {
  expect(shiftText("my-whoop", 5)).toBe("my-whoop")
  expect(shiftText("2026-11-28 07:12", 5)).toBe("2026-11-28 07:12")
  expect(shiftText("Pull day on 2026-11-28", 5)).toBe("Pull day on 2026-11-28")
})

test("every date in a response moves, except the birth date", () => {
  const shifted = shiftJson(
    {
      day: "2026-11-28",
      sleep: [{ start: "2026-11-27T23:25:00+01:00", minutes: 450 }],
      profile: { birth_date: "1992-03-14", height_cm: 181 },
      flags: [true, null],
    },
    2
  )

  expect(shifted).toEqual({
    day: "2026-11-30",
    sleep: [{ start: "2026-11-29T23:25:00+01:00", minutes: 450 }],
    profile: { birth_date: "1992-03-14", height_cm: 181 },
    flags: [true, null],
  })
})

test("whole days between two days follow the calendar", () => {
  expect(wholeDaysBetween("2026-11-28", "2026-11-28")).toBe(0)
  expect(wholeDaysBetween("2026-10-20", "2026-11-28")).toBe(39)
  expect(wholeDaysBetween("2026-11-28", "2026-11-20")).toBe(-8)
})

test("only Copenhagen offset form counts as normal", () => {
  expect(isNormalForm("2026-11-28T07:12:00+01:00")).toBe(true)
  expect(isNormalForm("2026-11-28T06:12:00Z")).toBe(false)
  expect(isNormalForm("2026-11-28")).toBe(true)
  expect(isNormalForm("2026-10-25T02:12:19+02:00")).toBe(true)
  expect(isNormalForm("2026-10-25T02:12:19+01:00")).toBe(true)
  expect(isNormalForm("2026-07-01T02:12:19+01:00")).toBe(false)
})
