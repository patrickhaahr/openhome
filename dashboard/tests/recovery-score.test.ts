import { expect, test } from "vitest"
import {
  chargeBand,
  clockDuration,
  restBand,
  ringShare,
  shortDuration,
  spokenDuration,
} from "@/lib/recovery-score"

test("bands Charge at WHOOP's 34 and 67 thresholds and keeps missing scores apart", () => {
  expect([null, 0, 33.9, 34, 66.9, 67, 100].map(chargeBand)).toEqual([
    "none",
    "poor",
    "poor",
    "fair",
    "fair",
    "good",
    "good",
  ])
})

test("bands Rest against the eight-hour reference", () => {
  expect([null, 184.7, 336, 407, 408, 600].map(restBand)).toEqual([
    "none",
    "poor",
    "fair",
    "fair",
    "good",
    "good",
  ])
})

test("clamps ring fills and formats sleep minutes for eyes and ears", () => {
  expect([
    ringShare(null, 100),
    ringShare(-5, 100),
    ringShare(150, 100),
  ]).toEqual([0, 0, 1])
  expect(clockDuration(184.68)).toBe("3:05")
  expect([shortDuration(32), shortDuration(65)]).toEqual(["32m", "1h 05m"])
  expect([spokenDuration(450), spokenDuration(60), spokenDuration(1)]).toEqual([
    "7 hours 30 minutes",
    "1 hour",
    "1 minute",
  ])
})
