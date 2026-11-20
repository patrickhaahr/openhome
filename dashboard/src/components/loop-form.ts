import type { Activity, LoopRequest } from "@/lib/route-loop"

type TargetChoice = "5k" | "10k" | "distance" | "steps"

export type Form = {
  readonly activity: Activity
  readonly target: TargetChoice
  readonly kilometers: string
  readonly steps: string
  readonly stepLength: Readonly<Record<Activity, string>>
}

export const INITIAL_FORM: Form = {
  activity: "walking",
  target: "5k",
  kilometers: "7.5",
  steps: "10000",
  stepLength: { walking: "75", running: "100" },
}

export type Field = "kilometers" | "steps" | "step-length"

export type Problem = { readonly field: Field; readonly message: string }

type FormRead =
  | { readonly state: "ready"; readonly request: LoopRequest }
  | ({ readonly state: "incomplete" } & Problem)

export function isTargetChoice(value: unknown): value is TargetChoice {
  return (
    value === "5k" ||
    value === "10k" ||
    value === "distance" ||
    value === "steps"
  )
}

export function readForm(form: Form): FormRead {
  const target = readTarget(form)

  if ("message" in target) return { state: "incomplete", ...target }

  const stepLength = wholeNumber(form.stepLength[form.activity])

  if (stepLength === undefined)
    return {
      state: "incomplete",
      field: "step-length",
      message: "Enter the step length in whole centimetres.",
    }

  return {
    state: "ready",
    request: { activity: form.activity, target, step_length_cm: stepLength },
  }
}

function readTarget(form: Form): LoopRequest["target"] | Problem {
  switch (form.target) {
    case "5k":
      return { kind: "distance", meters: 5000 }
    case "10k":
      return { kind: "distance", meters: 10_000 }
    case "distance": {
      const text = form.kilometers.trim().replace(",", ".")

      const meters = /^\d+(\.\d+)?$/.test(text)
        ? Math.round(Number(text) * 1000)
        : 0

      return Number.isSafeInteger(meters) && meters > 0
        ? { kind: "distance", meters }
        : { field: "kilometers", message: "Enter a distance in kilometres." }
    }

    case "steps": {
      const steps = wholeNumber(form.steps)

      return steps === undefined
        ? { field: "steps", message: "Enter a whole number of steps." }
        : { kind: "steps", steps }
    }
  }
}

function wholeNumber(text: string): number | undefined {
  const trimmed = text.trim()
  const value = /^\d+$/.test(trimmed) ? Number(trimmed) : 0

  return Number.isSafeInteger(value) && value > 0 ? value : undefined
}
