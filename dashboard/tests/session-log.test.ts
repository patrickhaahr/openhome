import { expect, test } from "vitest"
import { canPair, pairSessions } from "@/lib/session-log"

const workout = (id: number, date: string) => ({ id, date })

// Times are written in Copenhagen summer time (UTC+2).
const strap = (start: string, sport: string, minutes: number | null) => ({
  start: `${start}+02:00`,
  sport,
  duration_min: minutes,
})

function sessionLines(
  sessions: ReturnType<
    typeof pairSessions<ReturnType<typeof workout>, ReturnType<typeof strap>>
  >
): string[] {
  return sessions.map(
    (session) =>
      `${session.day} ${session.workout?.id ?? "-"} ${session.strap?.sport ?? "-"}`
  )
}

test("a logged workout and a strap session on the same day are one session", () => {
  const sessions = pairSessions(
    [workout(1, "2026-10-12")],
    [strap("2026-10-12T07:50:00", "Calisthenics", 52)]
  )

  expect(sessionLines(sessions)).toEqual(["2026-10-12 1 Calisthenics"])
})

test("a strap session on another day is a near miss, even a minute across midnight", () => {
  const sessions = pairSessions(
    [workout(1, "2026-10-12")],
    [strap("2026-10-13T00:01:00", "Calisthenics", 40)]
  )

  expect(sessionLines(sessions)).toEqual([
    "2026-10-13 - Calisthenics",
    "2026-10-12 1 -",
  ])
})

test("the Copenhagen day decides, not the UTC date", () => {
  // 00:30 local on the 13th is still the 12th in UTC.
  const sessions = pairSessions(
    [workout(1, "2026-10-13")],
    [strap("2026-10-13T00:30:00", "Calisthenics", 30)]
  )

  expect(sessionLines(sessions)).toEqual(["2026-10-13 1 Calisthenics"])
})

test("sports that cover ground never pair with a logged workout", () => {
  const sessions = pairSessions(
    [workout(1, "2026-10-12")],
    [
      strap("2026-10-12T07:00:00", "Running", 40),
      strap("2026-10-12T12:00:00", "Walking", 30),
    ]
  )

  expect(sessionLines(sessions)).toEqual([
    "2026-10-12 - Walking",
    "2026-10-12 - Running",
    "2026-10-12 1 -",
  ])
  expect(canPair("Running")).toBe(false)
  expect(canPair("Trail running")).toBe(false)
  expect(canPair("Calisthenics")).toBe(true)
  expect(canPair("Weightlifting")).toBe(true)
  expect(canPair("Functional fitness")).toBe(true)
})

test("two candidates for one workout: the longer session pairs, the other stands alone", () => {
  const sessions = pairSessions(
    [workout(1, "2026-10-12")],
    [
      strap("2026-10-12T07:00:00", "Mobility", 12),
      strap("2026-10-12T18:00:00", "Calisthenics", 55),
    ]
  )

  expect(sessionLines(sessions)).toEqual([
    "2026-10-12 1 Calisthenics",
    "2026-10-12 - Mobility",
  ])
})

test("equal lengths go to the earlier start", () => {
  const sessions = pairSessions(
    [workout(1, "2026-10-12")],
    [
      strap("2026-10-12T18:00:00", "Calisthenics", 45),
      strap("2026-10-12T07:00:00", "Calisthenics", 45),
    ]
  )

  expect(sessions.map((session) => session.strap?.start.slice(11, 16))).toEqual(
    ["18:00", "07:00"]
  )
  expect(sessions.map((session) => session.workout?.id)).toEqual([undefined, 1])
})

test("two workouts and two candidates pair in order, one to one", () => {
  const sessions = pairSessions(
    [workout(2, "2026-10-12"), workout(1, "2026-10-12")],
    [
      strap("2026-10-12T18:00:00", "Calisthenics", 40),
      strap("2026-10-12T07:00:00", "Calisthenics", 50),
    ]
  )

  expect(
    sessions.map((session) => [
      session.strap?.start.slice(11, 16),
      session.workout?.id,
    ])
  ).toEqual([
    ["18:00", 2],
    ["07:00", 1],
  ])
})

test("unpaired on both sides stay separate and the log runs newest first", () => {
  const sessions = pairSessions(
    [workout(1, "2026-10-10"), workout(2, "2026-10-12")],
    [
      strap("2026-10-12T07:50:00", "Calisthenics", 52),
      strap("2026-10-11T17:35:00", "Running", 34),
    ]
  )

  expect(sessionLines(sessions)).toEqual([
    "2026-10-12 2 Calisthenics",
    "2026-10-11 - Running",
    "2026-10-10 1 -",
  ])
})

test("a strap session with no duration still pairs", () => {
  const sessions = pairSessions(
    [workout(1, "2026-10-12")],
    [strap("2026-10-12T07:50:00", "Calisthenics", null)]
  )

  expect(sessionLines(sessions)).toEqual(["2026-10-12 1 Calisthenics"])
})
