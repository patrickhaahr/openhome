import { once } from "node:events"
import {
  createServer,
  type IncomingMessage,
  type Server,
  type ServerResponse,
} from "node:http"
import { z } from "zod"

/**
 * A proxy to Axum that forwards every request but, for the first request `lose` accepts,
 * lets Axum commit it and then drops the connection, as a lost response does.
 */
export async function startLossyAxum(
  upstream: string,
  lose: (request: IncomingMessage) => boolean
): Promise<{ url: string; stop: () => Promise<void> }> {
  let pending = true

  async function forward(
    request: IncomingMessage,
    response: ServerResponse
  ): Promise<void> {
    const body = await new Promise<string>((resolveBody) => {
      let text = ""
      request.on("data", (chunk: Buffer) => {
        text += chunk.toString()
      })
      request.on("end", () => resolveBody(text))
    })

    const answer = await fetch(`${upstream}${request.url ?? ""}`, {
      method: request.method ?? "GET",
      headers: {
        Authorization: request.headers.authorization ?? "",
        "Content-Type": "application/json",
      },
      body: body === "" ? null : body,
    })

    if (pending && lose(request)) {
      pending = false
      await answer.text()
      request.socket.destroy()

      return
    }

    response.writeHead(answer.status, { "Content-Type": "application/json" })
    response.end(await answer.text())
  }

  const server: Server = createServer((request, response) => {
    void forward(request, response).catch(() => response.destroy())
  })

  server.listen(0, "127.0.0.1")
  await once(server, "listening")

  const address = z.object({ port: z.number() }).parse(server.address())

  return {
    url: `http://127.0.0.1:${address.port}`,
    stop: async () => {
      server.closeAllConnections()
      await new Promise<void>((resolve, reject) =>
        server.close((error) => (error ? reject(error) : resolve()))
      )
    },
  }
}
