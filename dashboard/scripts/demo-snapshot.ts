import { spawn, type ChildProcess } from "node:child_process"
import { readFile, writeFile } from "node:fs/promises"
import { createServer, type Server } from "node:http"
import { createServer as createSocket } from "node:net"
import { join } from "node:path"
import { setTimeout as sleep } from "node:timers/promises"
import { z } from "zod"
import {
  isNormalForm,
  json,
  mapJson,
  type Json,
} from "../src/lib/demo/date-shift"
import { isDemoRoute, requestKey } from "../src/lib/demo/snapshot"
import { shiftDay, stripDays, trainingContext } from "../src/lib/training"
import { exercise, trainingDayOn, workoutSummary } from "../src/lib/workout"

type Recorded = Map<string, Json>

type StrapSession = { source: string; start: string; sport: string }

const apiKey = "synthetic-dashboard-api-key"

const dashboardDir = join(import.meta.dirname, "..")

const repoDir = join(dashboardDir, "..")

const apiBinary = join(repoDir, "api/target/debug/examples/dashboard_preview")

const outputPath = join(dashboardDir, "src/lib/demo/snapshot.json")

const trainingSeed = join(
  repoDir,
  "api/examples/dashboard_preview/seed/training.json"
)

const progressPeriods = ["4w", "3m", "12m"] as const

const bodyReaches = [30, 91, 365] as const

const historyStep = 20

const epochFloor = 1_000_000_000

const listening = z.object({ port: z.number() })

const seededDays = z.object({ days_ago: z.number() }).array()

const training = z.object({
  nights: seededDays,
  naps: seededDays,
  journal: seededDays,
  workouts: seededDays,
})

function freePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const probe = createSocket()

    probe.once("error", reject)
    probe.listen(0, "127.0.0.1", () => {
      const address = listening.safeParse(probe.address())

      probe.close(() =>
        address.success
          ? resolve(address.data.port)
          : reject(new Error("No free port"))
      )
    })
  })
}

async function until(
  what: string,
  attempts: number,
  ready: () => Promise<boolean>
): Promise<void> {
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    if (await ready().catch(() => false)) return
    await sleep(500)
  }

  throw new Error(`${what} never became ready`)
}

async function startRecordingProxy(
  upstream: string,
  recorded: Recorded
): Promise<{ port: number; server: Server }> {
  const port = await freePort()

  const server = createServer((incoming, outgoing) => {
    void (async () => {
      const url = new URL(incoming.url ?? "/", upstream)

      const response = await fetch(url, {
        method: incoming.method ?? "GET",
        headers: { Authorization: incoming.headers.authorization ?? "" },
        redirect: "manual",
      })

      const body = await response.text()

      if (
        incoming.method === "GET" &&
        isDemoRoute(url.pathname) &&
        response.status === 200
      )
        recorded.set(
          requestKey({
            path: url.pathname,
            query: Object.fromEntries(url.searchParams),
          }),
          json.parse(JSON.parse(body))
        )

      outgoing.writeHead(response.status, {
        "Content-Type": response.headers.get("content-type") ?? "text/plain",
      })
      outgoing.end(body)
    })().catch(() => {
      outgoing.writeHead(502)
      outgoing.end()
    })
  })

  await new Promise<void>((resolve) =>
    server.listen(port, "127.0.0.1", resolve)
  )

  return { port, server }
}

function spawnDetached(
  command: string,
  args: string[],
  cwd: string,
  env: Record<string, string>
): ChildProcess {
  return spawn(command, args, {
    cwd,
    env: { ...process.env, ...env },
    stdio: ["ignore", "ignore", "inherit"],
    detached: true,
  })
}

function stop(child: ChildProcess): void {
  if (child.pid !== undefined) process.kill(-child.pid, "SIGTERM")
}

async function oldestSeededDaysAgo(): Promise<number> {
  const seed = training.parse(JSON.parse(await readFile(trainingSeed, "utf8")))

  return Math.max(
    ...[...seed.nights, ...seed.naps, ...seed.journal, ...seed.workouts].map(
      (entry) => entry.days_ago
    )
  )
}

function strapSessions(recorded: Recorded): StrapSession[] {
  const found = new Map<string, StrapSession>()

  for (const [key, body] of recorded) {
    if (!key.startsWith("/api/training/context/")) continue

    for (const day of trainingContext.parse(body).days)
      for (const { source, start, sport } of day.noop_workouts)
        found.set(`${source}|${start}|${sport}`, { source, start, sport })
  }

  return [...found.values()]
}

function search(params: Record<string, string>): string {
  return `?${new URLSearchParams(params).toString()}`
}

function unmovable(recorded: Recorded): Json[] {
  const found: Json[] = []

  for (const body of recorded.values())
    mapJson(body, (_key, value) => {
      const text = z.string().safeParse(value)
      const number = z.number().safeParse(value)

      if (
        (text.success && !isNormalForm(text.data)) ||
        (number.success && number.data >= epochFloor)
      )
        found.push(value)

      return value
    })

  return found
}

function stopAll(cleanups: ReadonlyArray<() => void>): void {
  for (const cleanup of [...cleanups].reverse()) cleanup()
}

async function main(): Promise<void> {
  const capturedOn = trainingDayOn(new Date())

  const recorded: Recorded = new Map()

  const cleanups: Array<() => void> = []

  try {
    const apiPort = await freePort()

    const upstream = `http://127.0.0.1:${apiPort}`

    const api = spawnDetached(apiBinary, [], repoDir, {
      OPENHOME_PREVIEW_API_PORT: String(apiPort),
      OPENHOME_PREVIEW_COMPLETE_TODAY: "true",
      TMDB_READ_ACCESS_TOKEN: "",
    })

    cleanups.push(() => stop(api))

    await until("Synthetic API", 240, async () => {
      const health = await fetch(`${upstream}/api/health`, {
        headers: { Authorization: `Bearer ${apiKey}` },
      })

      return health.ok
    })

    const proxy = await startRecordingProxy(upstream, recorded)

    cleanups.push(() => proxy.server.close())

    const dashboardPort = await freePort()

    const dashboard = `http://127.0.0.1:${dashboardPort}`

    const server = spawnDetached(
      "bunx",
      [
        "vite",
        "dev",
        "--host",
        "127.0.0.1",
        "--port",
        String(dashboardPort),
        "--strictPort",
      ],
      dashboardDir,
      {
        OPENHOME_API_URL: `http://127.0.0.1:${proxy.port}`,
        API_KEY: apiKey,
      }
    )

    cleanups.push(() => stop(server))

    await until("Dashboard", 240, async () => {
      const response = await fetch(`${dashboard}/connection`)

      return response.ok
    })

    const visit = async (path: string): Promise<void> => {
      const response = await fetch(`${dashboard}${path}`, {
        headers: { Cookie: "openhome-data=live" },
      })

      if (!response.ok) throw new Error(`${path} answered ${response.status}`)
      await response.text()
    }

    const horizon = (await oldestSeededDaysAgo()) + 2 * stripDays

    await visit("/")
    await visit("/workout?view=progress")
    await visit("/workout?view=body")

    for (let ago = 0; ago <= horizon; ago += 1)
      await visit(`/recovery${search({ day: shiftDay(capturedOn, -ago) })}`)

    for (const session of strapSessions(recorded))
      await visit(
        `/recovery${search({
          day: session.start.slice(0, 10),
          noop: JSON.stringify(session),
        })}`
      )

    for (let shown = historyStep; ; shown += historyStep) {
      await visit(`/workout${search({ shown: String(shown) })}`)

      const history = workoutSummary.array().safeParse(
        recorded.get(
          requestKey({
            path: "/api/workouts",
            query: { limit: String(shown + 1) },
          })
        )
      )

      if (!history.success || history.data.length <= shown) break
    }

    const library = exercise
      .array()
      .parse(recorded.get(requestKey({ path: "/api/exercises", query: {} })))

    for (const period of progressPeriods) {
      await visit(`/workout${search({ view: "progress", period })}`)

      for (const { id } of library)
        await visit(
          `/workout${search({ view: "progress", period, compare: JSON.stringify([id]) })}`
        )
    }

    for (const reach of bodyReaches)
      await visit(
        `/workout${search({ view: "body", from: shiftDay(capturedOn, -reach) })}`
      )

    if (trainingDayOn(new Date()) !== capturedOn)
      throw new Error("The Copenhagen day changed during the capture; rerun it")

    const strange = unmovable(recorded)

    if (strange.length > 0)
      throw new Error(
        `Values the date shift cannot move: ${JSON.stringify(strange.slice(0, 3))}`
      )

    const lines = [...recorded]
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([key, body]) => `${JSON.stringify(key)}:${JSON.stringify(body)}`)

    await writeFile(
      outputPath,
      `{"capturedOn":${JSON.stringify(capturedOn)},"responses":{\n${lines.join(",\n")}\n}}\n`
    )
    console.log(`Recorded ${recorded.size} responses captured on ${capturedOn}`)
  } finally {
    stopAll(cleanups)
  }
}

await main()
