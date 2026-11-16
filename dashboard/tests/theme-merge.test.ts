import { readFileSync } from "node:fs"
import { expect, test } from "vitest"
import { cn } from "@/lib/utils"

const styles = readFileSync(
  new URL("../src/styles.css", import.meta.url),
  "utf8"
)

const tokens = (namespace: string): string[] =>
  [
    ...styles.matchAll(new RegExp(`^\\s*--${namespace}-([a-z0-9-]+):`, "gm")),
  ].flatMap(([, name]) =>
    name === undefined || name.includes("--") ? [] : [name]
  )

type Pair = readonly [string, string]

// Each token's utility must replace a built-in of its kind, and must keep a sibling of another
// kind that shares its prefix (`text-ink` beside `text-mega`).
type Case = (name: string) => { replaces: Pair; keeps: Pair | null }

const cases = {
  color: (name: string) => ({
    replaces: ["bg-black", `bg-${name}`],
    keeps: ["text-mega", `text-${name}`],
  }),
  text: (name: string) => ({
    replaces: ["text-sm", `text-${name}`],
    keeps: ["text-ink", `text-${name}`],
  }),
  leading: (name: string) => ({
    replaces: ["leading-4", `leading-${name}`],
    keeps: null,
  }),
  tracking: (name: string) => ({
    replaces: ["tracking-wide", `tracking-${name}`],
    keeps: null,
  }),
  spacing: (name: string) => ({ replaces: ["p-4", `p-${name}`], keeps: null }),
} satisfies Record<string, Case>

for (const [namespace, pairs] of Object.entries(cases)) {
  test.each(tokens(namespace))(
    `cn knows --${namespace}-%s from @theme`,
    (name: string) => {
      const { replaces, keeps } = pairs(name)

      expect(cn(...replaces)).toBe(replaces[1])

      if (keeps !== null) expect(cn(...keeps)).toBe(keeps.join(" "))
    }
  )
}
