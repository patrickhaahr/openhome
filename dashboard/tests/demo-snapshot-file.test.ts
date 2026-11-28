import { expect, test } from "vitest"
import { recoveryDay, shiftDay } from "@/lib/training"
import snapshotJson from "@/lib/demo/snapshot.json"
import { answerFromSnapshot, snapshotFile } from "@/lib/demo/snapshot"
import { workoutSummary } from "@/lib/workout"

const snapshot = snapshotFile.parse(snapshotJson)

test("the recorded snapshot parses", () => {
  expect(Object.keys(snapshot.responses).length).toBeGreaterThan(100)
})

test.each([0, 1, 9, 400])(
  "the home recovery glance is dated %i days after the capture",
  (later) => {
    const today = shiftDay(snapshot.capturedOn, later)

    const reply = answerFromSnapshot(
      snapshot,
      { path: `/api/training/days/${today}/recovery`, query: {} },
      today
    )

    expect(reply.state).toBe("found")

    const day = recoveryDay.parse(reply.state === "found" ? reply.body : null)

    expect(day.day).toBe(today)
    expect(day.derived_scores.recovery.value).not.toBeNull()
    expect(day.sleep.sessions[0]?.end.slice(0, 10)).toBe(today)
  }
)

test("the newest workout in the log is never dated after today", () => {
  const today = shiftDay(snapshot.capturedOn, 3)

  const reply = answerFromSnapshot(
    snapshot,
    { path: "/api/workouts", query: { limit: "21" } },
    today
  )

  const log = workoutSummary
    .array()
    .parse(reply.state === "found" ? reply.body : null)

  expect(log[0]?.date).toBe(today)
})
