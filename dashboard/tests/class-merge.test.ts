import { expect, test } from "vitest"
import { cn } from "@/lib/utils"

test("keeps a theme colour beside a theme font size", () => {
  expect(cn("text-ink", "text-mega")).toBe("text-ink text-mega")
  expect(cn("text-lead", "text-muted-ink")).toBe("text-lead text-muted-ink")
})

test("a later theme colour or size replaces only its own kind", () => {
  expect(cn("text-title", "text-ink", "text-sun")).toBe("text-title text-sun")
  expect(cn("text-title", "text-ink", "text-display")).toBe(
    "text-ink text-display"
  )
  expect(cn("bg-wash", "bg-wash-strong")).toBe("bg-wash-strong")
})

test("theme spacing merges with the built-in scale", () => {
  expect(cn("p-4", "p-margin")).toBe("p-margin")
  expect(cn("h-bar", "h-4")).toBe("h-4")
})
