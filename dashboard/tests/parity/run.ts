import { linkSync, mkdirSync, writeFileSync } from "node:fs"
import { join } from "node:path"
import { chromium, type Browser } from "playwright-core"
import { z } from "zod"
import { capture, type Job, type Shot } from "./capture.ts"
import { compare } from "./compare.ts"
import { flagPixels, line, writeReport, type Row } from "./report.ts"
import { scenarios, themes } from "./scenarios.ts"

function env(name: string): string {
  const value = process.env[name]

  if (value === undefined || value === "") throw new Error(`${name} is not set`)

  return value
}

const api = env("PARITY_API_URL")

const apiKey = env("PARITY_API_KEY")

const before = env("PARITY_BEFORE_URL")

const after = env("PARITY_AFTER_URL")

const dir = env("PARITY_RUN_DIR")

const filters = (process.env.PARITY_FILTER ?? "")
  .split(/\s+/)
  .filter((word) => word !== "")

const workers = Number(process.env.PARITY_WORKERS ?? "8")

const headers = {
  Authorization: `Bearer ${apiKey}`,
  "Content-Type": "application/json",
}

const owners = z.array(z.object({ id: z.string(), name: z.string() }))

const entries = z.object({
  entries: z.array(
    z.object({
      card: z.object({ card_value: z.object({ dkk: z.number() }).nullable() }),
    })
  ),
})

const additions = [
  ["OP99-001", "en"],
  ["OP99-001", "ja"],
  ["OP99-001_p1", "en"],
  ["OP99-002", "en"],
  ["OP99-002_p1", "en"],
  ["OP99-010", "en"],
  ["OP99-020", "en"],
  ["EB99_OP99-020_P1", "en"],
] as const

async function read<T>(path: string, schema: z.ZodType<T>): Promise<T> {
  const response = await fetch(api + path, { headers })

  if (!response.ok) throw new Error(`GET ${path}: ${response.status}`)

  return schema.parse(await response.json())
}

async function seedBinder(): Promise<void> {
  const alex = (await read("/api/household-users", owners)).find(
    (owner) => owner.name === "Alex"
  )

  if (alex === undefined) throw new Error("the synthetic API has no Alex")

  for (const [card, language] of additions) {
    const response = await fetch(`${api}/api/binder/entries`, {
      method: "POST",
      headers,
      body: JSON.stringify({
        owner_id: alex.id,
        card_id: card,
        language,
        request_id: `parity-${card}-${language}`,
      }),
    })

    if (!response.ok) throw new Error(`adding ${card}: ${response.status}`)
  }

  const deadline = Date.now() + 30_000

  for (;;) {
    const binder = await read(
      `/api/binder/entries?owner_id=${alex.id}`,
      entries
    )

    const priced = binder.entries.some(
      (entry) => entry.card.card_value !== null
    )

    if (binder.entries.length !== 20)
      throw new Error(
        `Alex has ${binder.entries.length} Binder entries, not 20`
      )

    if (priced) return

    if (Date.now() > deadline) throw new Error("card values never arrived")

    await new Promise((done) => setTimeout(done, 250))
  }
}

async function stylesheet(origin: string): Promise<string> {
  const html = await (await fetch(`${origin}/connection`)).text()

  return /assets\/(styles-[\w-]+\.css)/.exec(html)?.[1] ?? "unknown"
}

const blocked = new Set<string>()

async function shoot(browser: Browser, job: Job): Promise<[Shot, Shot]> {
  const now = Date.now()

  return Promise.all([
    capture(browser, before, job, now, blocked),
    capture(browser, after, job, now, blocked),
  ])
}

async function pair(browser: Browser, job: Job): Promise<Row> {
  const key = `${job.scenario.id}--${job.viewport}--${job.theme}`

  const base = {
    key,
    scenario: job.scenario.id,
    viewport: job.viewport,
    theme: job.theme,
  }

  let shots = await shoot(browser, job)
  let retried = false

  const differs = ([a, b]: [Shot, Shot]) =>
    a.state === "captured" && b.state === "captured" && !a.png.equals(b.png)

  if (differs(shots)) {
    shots = await shoot(browser, job)
    retried = true
  }

  const [a, b] = shots

  const problems = [
    ...a.problems.map((p) => `master: ${p}`),
    ...b.problems.map((p) => `branch: ${p}`),
  ]

  if (a.state === "failed" || b.state === "failed") {
    const reason = (["master", "branch"] as const)
      .flatMap((side, index) => {
        const shot = shots[index]

        return shot?.state === "failed" ? [`${side}: ${shot.reason}`] : []
      })
      .join("; ")

    return { ...base, outcome: { state: "failed", reason }, problems }
  }

  const folder = join(dir, "captures", key)
  mkdirSync(folder, { recursive: true })
  const { diff, ...comparison } = compare(a.png, b.png)
  writeFileSync(join(folder, "before.png"), a.png)

  if (diff === null)
    linkSync(join(folder, "before.png"), join(folder, "after.png"))
  else {
    writeFileSync(join(folder, "after.png"), b.png)
    writeFileSync(join(folder, "diff.png"), diff)
  }

  const flagged = comparison.pixels >= flagPixels || comparison.sizeChanged

  return {
    ...base,
    outcome: { state: "compared", comparison, flagged, retried },
    problems,
  }
}

const jobs: Job[] = scenarios
  .filter(
    (scenario) =>
      filters.length === 0 || filters.some((word) => scenario.id.includes(word))
  )
  .flatMap((scenario) =>
    scenario.viewports.flatMap((viewport) =>
      themes.map((theme) => ({ scenario, viewport, theme }))
    )
  )

if (jobs.length === 0)
  throw new Error(`no scenario id matches ${filters.join(" ")}`)

const startedAt = new Date()

await seedBinder()

const browser = await chromium.launch({
  executablePath: env("PARITY_CHROMIUM"),
  args: ["--font-render-hinting=none", "--disable-partial-raster"],
})

const rows: Row[] = []

const queue = [...jobs]

console.log(`Capturing ${jobs.length} pairs with ${workers} workers`)

await Promise.all(
  Array.from({ length: workers }, async () => {
    for (let job = queue.shift(); job !== undefined; job = queue.shift()) {
      const row = await pair(browser, job)
      rows.push(row)
      console.log(line(row))
    }
  })
)

await browser.close()

const header = {
  run: env("PARITY_RUN"),
  master: env("PARITY_MASTER"),
  branch: env("PARITY_BRANCH"),
  css: { before: await stylesheet(before), after: await stylesheet(after) },
  startedAt: startedAt.toISOString(),
  durationMs: Date.now() - startedAt.getTime(),
  blockedHosts: [...blocked].sort(),
}

writeReport(dir, header, rows)

const failed = rows.filter(
  (row) => row.outcome.state === "failed" || row.problems.length > 0
)

const flagged = rows.filter(
  (row) => row.outcome.state === "compared" && row.outcome.flagged
)

const changed = rows.filter(
  (row) => row.outcome.state === "compared" && row.outcome.comparison.pixels > 0
)

console.log(`\nmaster ${header.master}  branch ${header.branch}`)

console.log(`css ${header.css.before} -> ${header.css.after}`)

console.log(`blocked hosts: ${header.blockedHosts.join(", ") || "none"}`)

console.log(
  `${rows.length} captures, ${changed.length} changed, ${flagged.length} flagged, ${failed.length} with errors in ${Math.round(header.durationMs / 1000)} s`
)

console.log(`Report: ${join(dir, "index.html")}`)

if (failed.length > 0) process.exitCode = 1
