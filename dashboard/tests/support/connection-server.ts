import { spawn, type ChildProcess } from "node:child_process"
import type { Server } from "node:http"
import { once } from "node:events"
import { fileURLToPath } from "node:url"
import { afterAll, afterEach, beforeAll, expect } from "vitest"
import { z } from "zod"

const root = fileURLToPath(new URL("../../", import.meta.url))

export const apiKey = "synthetic-dashboard-api-key"

const processes: ChildProcess[] = []

const servers: Server[] = []

export let axumUrl: string

async function startProcess({
  command,
  args,
  env,
}: {
  command: string
  args: string[]
  env: NodeJS.ProcessEnv
}): Promise<string> {
  const child = spawn(command, args, {
    cwd: root,
    env,
    stdio: ["ignore", "pipe", "pipe"],
  })

  processes.push(child)

  return new Promise((resolve, reject) => {
    let output = ""

    const timeout = setTimeout(
      () => reject(new Error(`Server startup timed out: ${output}`)),
      20000
    )

    child.stdout.on("data", (chunk: Buffer) => {
      output += chunk.toString()
      const url = output.match(/http:\/\/127\.0\.0\.1:\d+/)?.[0]

      if (url !== undefined) {
        clearTimeout(timeout)
        resolve(url)
      }
    })
    child.stderr.on("data", (chunk: Buffer) => {
      output += chunk.toString()
    })
    child.once("error", (error) => {
      clearTimeout(timeout)
      reject(error)
    })
    child.once("exit", (code) => {
      clearTimeout(timeout)
      reject(new Error(`Server exited (${code}): ${output}`))
    })
  })
}

export async function dashboard({
  url = "",
  key = "",
}: { url?: string; key?: string } = {}): Promise<string> {
  return startProcess({
    command: "bun",
    args: [
      "node_modules/vite/bin/vite.js",
      "dev",
      "--host",
      "127.0.0.1",
      "--port",
      "0",
      "--clearScreen",
      "false",
    ],
    env: { ...process.env, OPENHOME_API_URL: url, API_KEY: key },
  })
}

export async function page(url: string, path = "/connection"): Promise<string> {
  const response = await fetch(`${url}${path}`)
  expect(response.status).toBe(200)
  expect(response.headers.get("cache-control")).toBe("no-store")
  const html = await response.text()
  expect(html).not.toContain(apiKey)
  expect(html).not.toContain(axumUrl)

  return html
}

beforeAll(async () => {
  axumUrl = await startProcess({
    command: fileURLToPath(
      new URL(
        "../../../api/target/debug/examples/dashboard_health",
        import.meta.url
      )
    ),
    args: [],
    env: process.env,
  })
})

async function cleanup(children: ChildProcess[]): Promise<void> {
  for (const child of children) {
    if (child.exitCode === null && child.signalCode === null) {
      const exited = once(child, "exit")
      child.kill("SIGTERM")
      await exited
    }
  }

  for (const server of servers.splice(0)) {
    server.closeAllConnections()
    await new Promise<void>((resolve, reject) => {
      server.close((error) => {
        if (error !== undefined) reject(error)
        else resolve()
      })
    })
  }
}

afterEach(() => cleanup(processes.splice(1)))

afterAll(() => cleanup(processes.splice(0)))

export async function listen(server: Server): Promise<string> {
  servers.push(server)
  server.listen(0, "127.0.0.1")
  await once(server, "listening")
  const address = z.object({ port: z.number() }).parse(server.address())

  return `http://127.0.0.1:${address.port}`
}
