import { spawn } from "node:child_process"
import { once } from "node:events"
import { resolve } from "node:path"

/**
 * Production authenticated routes over an isolated migrated synthetic database.
 *
 * @param args - Fixture options, such as `--food-database=importing`.
 * @returns The fixture's loopback URL and a stop function.
 */
export async function startAxumFixture(args: readonly string[] = []): Promise<{
  url: string
  stop: () => Promise<void>
}> {
  const child = spawn(
    resolve("../api/target/debug/examples/dashboard_health"),
    [...args],
    {
      stdio: ["ignore", "pipe", "pipe"],
    }
  )

  async function stop(): Promise<void> {
    if (child.exitCode === null && child.signalCode === null) {
      const exited = once(child, "exit")
      child.kill("SIGTERM")
      await exited
    }
  }

  try {
    const url = await new Promise<string>((resolveUrl, reject) => {
      const timeout = setTimeout(
        () => reject(new Error("Axum startup timed out")),
        20000
      )

      child.stdout.on("data", (chunk: Buffer) => {
        const url = chunk.toString().match(/http:\/\/127\.0\.0\.1:\d+/)?.[0]

        if (url !== undefined) {
          clearTimeout(timeout)
          resolveUrl(url)
        }
      })
      child.once("error", (error) => {
        clearTimeout(timeout)
        reject(error)
      })
      child.once("exit", (code) => {
        clearTimeout(timeout)
        reject(new Error(`Axum exited: ${code}`))
      })
    })

    return { url, stop }
  } catch (error) {
    await stop()
    throw error
  }
}
