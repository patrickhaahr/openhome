import { once } from "node:events"
import { createServer, type Server } from "node:http"
import { afterEach, expect, test, vi } from "vitest"
import { z } from "zod"
import { dispatchTvCommand, readTvRemote } from "@/lib/tv.server"

const servers: Server[] = []

const key = "synthetic-tv-api-key"

/** Configure the dashboard server to reach OpenHome at this server. */
async function listenAsOpenHome(server: Server): Promise<void> {
  servers.push(server)
  server.listen(0, "127.0.0.1")
  await once(server, "listening")
  const { port } = z.object({ port: z.number() }).parse(server.address())
  vi.stubEnv("OPENHOME_API_URL", `http://127.0.0.1:${port}/deployment/`)
  vi.stubEnv("API_KEY", key)
}

function respondWith(status: number, body: string): Server {
  return createServer((_request, response) => {
    response.writeHead(status, { "Content-Type": "application/json" })
    response.end(body)
  })
}

afterEach(async () => {
  vi.unstubAllEnvs()

  for (const server of servers.splice(0)) {
    server.closeAllConnections()
    await new Promise<void>((resolve, reject) => {
      server.close((error) => {
        if (error !== undefined) reject(error)
        else resolve()
      })
    })
  }
})

test("discovers only reported TV commands that have Remote Buttons, in remote order", async () => {
  const requests: { path: string | undefined; authorization?: string }[] = []

  await listenAsOpenHome(
    createServer((request, response) => {
      requests.push({
        path: request.url,
        authorization: request.headers.authorization ?? "",
      })
      response.writeHead(200, { "Content-Type": "application/json" })
      response.end(
        JSON.stringify({
          message: "synthetic-private-provider-detail",
          remotes: {
            edifier: ["bluetooth"],
            lgtv: ["hdmi-1", "synthetic-unknown", "mute", "power"],
          },
        })
      )
    })
  )

  expect(await readTvRemote()).toEqual({
    state: "available",
    commands: ["power", "mute", "hdmi-1"],
  })
  expect(requests).toEqual([
    { path: "/deployment/api/ir", authorization: `Bearer ${key}` },
  ])
})

test.each([
  {
    status: 503,
    body: '{"error":"IR service is not configured"}',
    reason: "missing-device",
  },
  {
    status: 503,
    body: '{"error":"synthetic-private-bridge-offline"}',
    reason: "unavailable",
  },
  {
    status: 401,
    body: '{"error":"synthetic-private-credential-error"}',
    reason: "rejected-credential",
  },
  { status: 200, body: '{"remotes":{}}', reason: "unavailable" },
  {
    status: 200,
    body: "synthetic-private-invalid-json",
    reason: "unavailable",
  },
])(
  "reports unavailable discovery without private detail ($status, $reason)",
  async ({ status, body, reason }) => {
    await listenAsOpenHome(respondWith(status, body))

    expect(await readTvRemote()).toEqual({ state: "unavailable", reason })
  }
)

test("sends one authenticated JSON command to the LG TV remote", async () => {
  const requests: { method?: string; path?: string; body: string }[] = []

  await listenAsOpenHome(
    createServer((request, response) => {
      let body = ""
      request.on("data", (chunk: Buffer) => {
        body += chunk.toString()
      })
      request.on("end", () => {
        requests.push({
          method: request.method ?? "",
          path: request.url ?? "",
          body,
        })
        response.writeHead(200, { "Content-Type": "application/json" })
        response.end('{"message":"synthetic-private-provider-detail"}')
      })
    })
  )

  expect(await dispatchTvCommand("volume-up")).toEqual({ state: "accepted" })
  expect(requests).toEqual([
    {
      method: "POST",
      path: "/deployment/api/ir/lgtv",
      body: '{"command":"volume-up"}',
    },
  ])
})

test.each([
  {
    status: 404,
    body: '{"error":"Unknown command"}',
    reason: "unsupported-command",
  },
  {
    status: 404,
    body: '{"error":"Unknown remote"}',
    reason: "unavailable",
  },
  {
    status: 404,
    body: "synthetic-private-missing-route",
    reason: "unavailable",
  },
  {
    status: 503,
    body: '{"error":"IR service is not configured"}',
    reason: "missing-device",
  },
  {
    status: 503,
    body: '{"error":"synthetic-private-bridge-offline"}',
    reason: "unavailable",
  },
  {
    status: 500,
    body: '{"error":"synthetic-private-internal"}',
    reason: "unavailable",
  },
  {
    status: 200,
    body: '{"power":"on","detail":"synthetic-private-state"}',
    reason: "unavailable",
  },
])(
  "sanitizes unconfirmed TV commands ($status, $reason)",
  async ({ status, body, reason }) => {
    await listenAsOpenHome(respondWith(status, body))

    expect(await dispatchTvCommand("power")).toEqual({
      state: "failed",
      reason,
    })
  }
)

test("keeps missing dashboard setup distinct from a missing TV remote", async () => {
  vi.stubEnv("OPENHOME_API_URL", "")
  vi.stubEnv("API_KEY", "")
  expect(await readTvRemote()).toEqual({
    state: "unavailable",
    reason: "missing-configuration",
  })
  expect(await dispatchTvCommand("power")).toEqual({
    state: "failed",
    reason: "missing-configuration",
  })
})
