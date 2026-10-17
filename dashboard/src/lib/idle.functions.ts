import { createIsomorphicFn } from "@tanstack/react-start"
import { parseIdleSettings, type IdleSettings } from "./idle"

/**
 * The idle choices, read from the server environment while the first page renders. Client
 * navigations return null: the shell keeps what the server sent, without another request.
 */
export const readIdleSettings = createIsomorphicFn()
  .server((): IdleSettings | null =>
    parseIdleSettings(
      process.env.IDLE_TIMEOUT_SECONDS,
      process.env.IDLE_PREVIEW
    )
  )
  .client((): IdleSettings | null => null)
