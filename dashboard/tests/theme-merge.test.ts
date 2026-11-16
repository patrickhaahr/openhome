import { readdirSync, readFileSync } from "node:fs"
import { expect, test } from "vitest"
import { cn } from "@/lib/utils"

const source = new URL("../src/", import.meta.url)

// App-wide tokens live in styles.css, each area's own in <area>.theme.css beside it.
const styles = readdirSync(source)
  .filter((file) => file === "styles.css" || file.endsWith(".theme.css"))
  .map((file) => readFileSync(new URL(file, source), "utf8"))
  .join("\n")

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

test("@theme declares each token once", () => {
  const names = [
    ...styles.matchAll(
      /^\s*(--(?:color|text|leading|tracking|spacing|radius|inset|grid-template-[a-z]+|animate|ease|shadow)-[a-z0-9-]+):/gm
    ),
  ].map(([, name]) => name)

  expect(names.filter((name, index) => names.indexOf(name) !== index)).toEqual(
    []
  )
})
