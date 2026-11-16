import { writeFileSync } from "node:fs"
import { join } from "node:path"
import type { Comparison } from "./compare.ts"
import type { Theme, ViewportName } from "./scenarios.ts"

/** A scenario is worth a look at this many differing pixels, or at any change of size. */
export const flagPixels = 50

export type Outcome =
  | {
      readonly state: "compared"
      readonly comparison: Omit<Comparison, "diff">
      readonly flagged: boolean
      /** Compared a second time after a first non-zero result; the second result stands. */
      readonly retried: boolean
    }
  | { readonly state: "failed"; readonly reason: string }

export type Row = {
  readonly key: string
  readonly scenario: string
  readonly viewport: ViewportName
  readonly theme: Theme
  readonly outcome: Outcome
  readonly problems: readonly string[]
}

export type Header = {
  readonly run: string
  readonly master: string
  readonly branch: string
  readonly css: { readonly before: string; readonly after: string }
  readonly startedAt: string
  readonly durationMs: number
  readonly blockedHosts: readonly string[]
}

const percent = (comparison: Omit<Comparison, "diff">) =>
  comparison.total === 0 ? 0 : (comparison.pixels / comparison.total) * 100

/** Failures first, then the most differing pixels. */
function severity(row: Row): number {
  return row.outcome.state === "failed"
    ? Number.POSITIVE_INFINITY
    : row.outcome.comparison.pixels + (row.outcome.flagged ? 1e12 : 0)
}

export function line(row: Row): string {
  const warnings =
    row.problems.length === 0 ? "" : `  (${row.problems.join("; ")})`

  if (row.outcome.state === "failed")
    return `   FAILED            ${row.key}: ${row.outcome.reason}${warnings}`

  const { comparison, flagged, retried } = row.outcome

  const marks = [
    flagged ? "FLAG" : "",
    comparison.sizeChanged ? "size" : "",
    retried ? "retried" : "",
  ]
    .join(" ")
    .trim()

  return `${String(comparison.pixels).padStart(9)} px ${percent(comparison).toFixed(3).padStart(7)}%  ${row.key}${marks === "" ? "" : `  ${marks}`}${warnings}`
}

const escape = (text: string) =>
  text
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll('"', "&quot;")

function figure(row: Row, name: string): string {
  const src = `captures/${row.key}/${name}.png`

  return `<figure><figcaption>${name}</figcaption><a href="${src}"><img loading="lazy" src="${src}" alt="${name} of ${escape(row.key)}"></a></figure>`
}

function card(row: Row): string {
  const changed =
    row.outcome.state === "compared" && row.outcome.comparison.pixels > 0

  const images =
    row.outcome.state === "failed"
      ? ""
      : [
          figure(row, "before"),
          figure(row, "after"),
          changed ? figure(row, "diff") : "",
        ].join("")

  const problems = row.problems
    .map((problem) => `<li>${escape(problem)}</li>`)
    .join("")

  return `<section class="${row.outcome.state === "compared" && row.outcome.flagged ? "flag" : row.outcome.state}"><h2>${escape(line(row))}</h2>${problems === "" ? "" : `<ul>${problems}</ul>`}<div>${images}</div></section>`
}

/** One capture in `summary.json`, the same keys whatever its state so readers need no branching. */
function entry(row: Row) {
  const compared = row.outcome.state === "compared" ? row.outcome : null

  return {
    key: row.key,
    scenario: row.scenario,
    viewport: row.viewport,
    theme: row.theme,
    state: row.outcome.state,
    pixels: compared?.comparison.pixels ?? null,
    percent: compared === null ? null : percent(compared.comparison),
    flagged: compared?.flagged ?? true,
    sizeChanged: compared?.comparison.sizeChanged ?? false,
    retried: compared?.retried ?? false,
    before: compared?.comparison.before ?? null,
    after: compared?.comparison.after ?? null,
    reason: row.outcome.state === "failed" ? row.outcome.reason : null,
    problems: row.problems,
  }
}

/** Writes `summary.json` for agents and `index.html` for people, both worst first. */
export function writeReport(
  dir: string,
  header: Header,
  rows: readonly Row[]
): void {
  const sorted = [...rows].sort(
    (a, b) => severity(b) - severity(a) || a.key.localeCompare(b.key)
  )

  const captures = sorted.map(entry)

  const totals = {
    captures: rows.length,
    scenarios: new Set(rows.map((row) => row.scenario)).size,
    changed: captures.filter((row) => row.pixels !== 0).length,
    flagged: captures.filter((row) => row.flagged).length,
    failed: captures.filter((row) => row.state === "failed").length,
    withProblems: rows.filter((row) => row.problems.length > 0).length,
  }

  writeFileSync(
    join(dir, "summary.json"),
    `${JSON.stringify({ ...header, flagPixels, totals, captures }, null, 2)}\n`
  )

  const facts = [
    `master ${header.master}`,
    `branch ${header.branch}`,
    `css ${header.css.before} → ${header.css.after}`,
    `${totals.captures} captures of ${totals.scenarios} scenarios in ${Math.round(header.durationMs / 1000)} s`,
    `${totals.changed} changed, ${totals.flagged} flagged (≥ ${flagPixels} px or resized), ${totals.failed} failed`,
    `blocked hosts: ${header.blockedHosts.join(", ") || "none"}`,
  ]

  writeFileSync(
    join(dir, "index.html"),
    `<!doctype html><meta charset="utf-8"><title>Dashboard parity ${escape(header.run)}</title>
<style>body{font:14px system-ui;margin:1.5rem;background:#f4f4f2}h2{font:13px ui-monospace,monospace;white-space:pre;margin:.25rem 0}
section{background:#fff;border-left:4px solid #ccc;padding:.5rem 1rem;margin:0 0 1rem}section.flag,section.failed{border-color:#d22}
section div{display:flex;gap:1rem;align-items:flex-start}figure{margin:0;flex:1}img{width:100%;max-height:60vh;object-fit:contain;object-position:top;border:1px solid #ddd}</style>
<h1>Dashboard parity ${escape(header.run)}</h1><ul>${facts.map((fact) => `<li>${escape(fact)}</li>`).join("")}</ul>
${sorted.map(card).join("\n")}\n`
  )
}
