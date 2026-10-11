import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import { tvCommand, type TvRemoteState } from "./tv"

/** Discover available TV commands on the server; browsers receive only known command names. */
export const getTvRemote = createServerFn({ method: "GET" }).handler(
  async () => {
    setResponseHeader("Cache-Control", "no-store")
    const { readTvRemote } = await import("./tv.server")

    return readTvRemote()
  }
)

/** Send one TV Command through Axum on the server; browsers receive only a sanitized outcome. */
export const sendTvCommand = createServerFn({ method: "POST" })
  .validator(tvCommand)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { dispatchTvCommand } = await import("./tv.server")

    return dispatchTvCommand(data)
  })

/**
 * Discover TV commands for a route loader without letting a dropped RPC reject the page.
 *
 * @returns The discovered remote, or an unreachable outcome when the dashboard server did not answer.
 */
export async function loadTvRemote(): Promise<TvRemoteState> {
  try {
    return await getTvRemote()
  } catch {
    return { state: "unavailable", reason: "unreachable" }
  }
}
