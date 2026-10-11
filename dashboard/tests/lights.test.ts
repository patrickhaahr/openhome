import { once } from "node:events"
import { createServer, type Server } from "node:http"
import { afterEach, expect, test, vi } from "vitest"
import { z } from "zod"
import { dispatchLightCommand } from "@/lib/lights.server"

const servers: Server[] = []

const key = "synthetic-lights-api-key"

async function listen(server: Server): Promise<string> {
  servers.push(server)
  server.listen(0, "127.0.0.1")
  await once(server, "listening")
  const { port } = z.object({ port: z.number() }).parse(server.address())

  return `http://127.0.0.1:${port}`
}

/** Configure the dashboard server to reach OpenHome at this server. */
async function listenAsOpenHome(server: Server): Promise<void> {
  const url = await listen(server)
  vi.stubEnv("OPENHOME_API_URL", `${url}/deployment/`)
  vi.stubEnv("API_KEY", key)
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

test("sends each supported command once with server-held authentication and preserves the deployment prefix", async () => {
  const requests: {
    method: string | undefined
    path: string | undefined
    authorization: string | undefined
  }[] = []

  const upstream = createServer((request, response) => {
    requests.push({
      method: request.method,
      path: request.url,
      authorization: request.headers.authorization,
    })
    response.writeHead(200, { "Content-Type": "application/json" })
    response.end(
      JSON.stringify({ message: "synthetic-private-provider-detail" })
    )
  })

  await listenAsOpenHome(upstream)

  expect(await dispatchLightCommand("on")).toEqual({ state: "accepted" })
  expect(await dispatchLightCommand("off")).toEqual({ state: "accepted" })
  expect(requests).toEqual([
    {
      method: "POST",
      path: "/deployment/api/lights/on",
      authorization: `Bearer ${key}`,
    },
    {
      method: "POST",
      path: "/deployment/api/lights/off",
      authorization: `Bearer ${key}`,
    },
  ])
})

test.each([
  {
    status: 503,
    body: '{"error":"SwitchBot service is not configured"}',
    reason: "missing-device",
  },
  {
    status: 503,
    body: '{"error":"synthetic-private-provider-error"}',
    reason: "unavailable",
  },
  {
    status: 401,
    body: '{"error":"synthetic-private-credential-error"}',
    reason: "rejected-credential",
  },
  {
    status: 200,
    body: "synthetic-private-invalid-json",
    reason: "unavailable",
  },
  {
    status: 200,
    body: '{"power":"on","detail":"synthetic-private-state"}',
    reason: "unavailable",
  },
])(
  "sanitizes unconfirmed command responses ($status, $reason)",
  async ({ status, body, reason }) => {
    await listenAsOpenHome(
      createServer((_request, response) => {
        response.writeHead(status, { "Content-Type": "application/json" })
        response.end(body)
      })
    )

    expect(await dispatchLightCommand("on")).toEqual({
      state: "failed",
      reason,
    })
  }
)

test("keeps missing dashboard setup distinct from missing lights", async () => {
  vi.stubEnv("OPENHOME_API_URL", "")
  vi.stubEnv("API_KEY", "")
  expect(await dispatchLightCommand("on")).toEqual({
    state: "failed",
    reason: "missing-configuration",
  })
})

test("does not follow redirects that could expose the API key or replay commands", async () => {
  const destinationRequests: string[] = []

  const destination = await listen(
    createServer((request, response) => {
      destinationRequests.push(request.url ?? "")
      response.end('{"message":"accepted"}')
    })
  )

  await listenAsOpenHome(
    createServer((_request, response) => {
      response.writeHead(307, { Location: `${destination}/other-device` })
      response.end()
    })
  )

  expect(await dispatchLightCommand("on")).toEqual({
    state: "failed",
    reason: "unreachable",
  })
  expect(destinationRequests).toEqual([])
})

test("bounds a stalled command without automatically retrying it", async () => {
  let requests = 0
  await listenAsOpenHome(
    createServer(() => {
      requests += 1
    })
  )
  const started = Date.now()

  expect(await dispatchLightCommand("off")).toEqual({
    state: "failed",
    reason: "unreachable",
  })
  expect(Date.now() - started).toBeLessThan(15000)
  expect(requests).toBe(1)
})
