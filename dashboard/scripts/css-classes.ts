export type PlainClass = { readonly line: number; readonly selector: string }

/** Theme switch toggled on <html> by lib/daylight.ts; the tokens themselves live under it. */
const themeClasses = new Set(["dark"])

type Block = "utility" | "opaque" | "group" | "rule"

/** Replaces comments and string contents with spaces, keeping newlines so line numbers survive. */
function blank(css: string): string {
  let out = ""
  let index = 0

  while (index < css.length) {
    const char = css.charAt(index)
    const next = css.charAt(index + 1)

    if (char === "/" && next === "*") {
      const end = css.indexOf("*/", index + 2)
      const stop = end === -1 ? css.length : end + 2
      out += css.slice(index, stop).replace(/[^\n]/g, " ")
      index = stop
    } else if (char === '"' || char === "'") {
      let stop = index + 1

      while (stop < css.length && css.charAt(stop) !== char) {
        stop += css.charAt(stop) === "\\" ? 2 : 1
      }

      out += `${char}${css.slice(index + 1, stop).replace(/[^\n]/g, " ")}${char}`
      index = stop + 1
    } else {
      out += char
      index += 1
    }
  }

  return out
}

function classesOf(selector: string): string[] {
  const withoutAttributes = selector.replace(/\[[^\]]*\]/g, "")
  const names = [...withoutAttributes.matchAll(/\.((?:\\.|[\w-])+)/g)]

  return names.map((match) => match[1] ?? "")
}

function kindOf(prelude: string): Block {
  const head = /^@([\w-]+)/.exec(prelude)?.[1]?.replace(/^-\w+-/, "")

  if (head === undefined) return "rule"

  if (head === "utility") return "utility"

  if (head === "keyframes") return "opaque"

  return "group"
}

/** Class selectors written as rules outside any `@utility` block, with the line each rule starts on. */
export function plainClasses(css: string): PlainClass[] {
  const text = blank(css)
  const found: PlainClass[] = []
  const stack: Block[] = []
  let start = 0
  let line = 1
  let firstLine = 0

  for (let index = 0; index < text.length; index += 1) {
    const char = text.charAt(index)

    if (char === "\n") line += 1
    else if (
      firstLine === 0 &&
      char.trim() !== "" &&
      char !== "}" &&
      char !== ";"
    )
      firstLine = line

    if (char === "(" && /\burl$/.test(text.slice(0, index))) {
      index = text.indexOf(")", index)

      if (index === -1) break
    } else if (char === "{") {
      const prelude = text.slice(start, index).trim()
      const hidden = stack.includes("utility") || stack.includes("opaque")
      const kind = kindOf(prelude)
      const named = classesOf(prelude).filter((name) => !themeClasses.has(name))

      if (!hidden && kind === "rule" && named.length > 0)
        found.push({ line: firstLine, selector: prelude.replace(/\s+/g, " ") })
      stack.push(kind)
      start = index + 1
      firstLine = 0
    } else if (char === ";" || char === "}") {
      if (char === "}") stack.pop()
      start = index + 1
      firstLine = 0
    }
  }

  return found
}

/**
 * Problems across stylesheets, keyed by path: plain classes in files off the allowlist, and
 * allowlist entries whose file is gone, so area agents must remove an entry when they delete a sheet.
 */
export function audit(
  sources: ReadonlyMap<string, string>,
  allowlist: readonly string[]
): string[] {
  const problems: string[] = []

  for (const [path, css] of sources) {
    if (allowlist.includes(path)) continue

    for (const hit of plainClasses(css))
      problems.push(
        `${path}:${hit.line} plain class selector outside @utility: ${hit.selector}`
      )
  }

  for (const path of allowlist)
    if (!sources.has(path))
      problems.push(
        `${path} is on the allowlist but no longer exists; remove it from scripts/check-css.ts`
      )

  return problems
}
