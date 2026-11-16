import { expect, test } from "vitest"
import { audit, plainClasses } from "../scripts/css-classes.ts"

const selectors = (css: string) => plainClasses(css).map((hit) => hit.selector)

test("reports a plain class with the line its rule starts on", () => {
  expect(
    plainClasses("a {\n  color: red;\n}\n\n.probe {\n  color: red;\n}\n")
  ).toEqual([{ line: 5, selector: ".probe" }])
})

test("ignores element rules, the theme switch, @theme and @layer base", () => {
  const css = `@theme inline { --color-ink: var(--ink); }
:root { --ink: #000; }
.dark { --ink: #fff; }
@custom-variant dark (&:is(.dark *));
@layer base { html { color: red; } .dark body { color: blue; } }`

  expect(selectors(css)).toEqual([])
})

test("finds classes inside nested at-rules and selector functions", () => {
  const css = `@layer components { .a { color: red; } }
@media (min-width: 600px) { @supports (display: grid) { .b > .c { display: grid; } } }
section:is(.d, .e):has(.f) { margin: 0; }
:where(.g) { margin: 0; }`

  expect(selectors(css)).toEqual([
    ".a",
    ".b > .c",
    "section:is(.d, .e):has(.f)",
    ":where(.g)",
  ])
})

test("allows anything inside @utility, however deeply nested", () => {
  const css = `@utility card {
  display: grid;
  > .inner { color: red; }
  @media (width > 40rem) { .inner:hover { color: blue; } }
  &:is(.on) { color: green; }
}
.after { color: red; }`

  expect(plainClasses(css)).toEqual([{ line: 7, selector: ".after" }])
})

test("skips comments, strings, attribute selectors and keyframe steps", () => {
  const css = `/* .commented { color: red; } */
a[href$=".pdf"]::after { content: ".quoted { x: y }"; background: url(a.png); }
@keyframes spin { from { opacity: 0; } 50.5% { opacity: 1; } }
@-webkit-keyframes spin { to { opacity: 0; } }`

  expect(selectors(css)).toEqual([])
})

test("handles escaped class names and CSS nesting", () => {
  expect(selectors(".sm\\:flex { display: flex; }")).toEqual([".sm\\:flex"])
  expect(selectors("main { .nested { color: red; } }")).toEqual([".nested"])
})

test("audit exempts allowlisted files and flags missing ones", () => {
  const sources = new Map([
    ["src/old.css", ".a { color: red; }"],
    ["src/new.css", ".probe { color: red; }"],
  ])

  expect(audit(sources, ["src/old.css", "src/gone.css"])).toEqual([
    "src/new.css:1 plain class selector outside @utility: .probe",
    "src/gone.css is on the allowlist but no longer exists; remove it from scripts/check-css.ts",
  ])
})
