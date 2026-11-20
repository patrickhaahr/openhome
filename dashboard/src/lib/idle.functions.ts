import { createIsomorphicFn } from "@tanstack/react-start"
import { parseIdleSettings, type IdleSettings } from "./idle"

export const readIdleSettings = createIsomorphicFn()
  .server((): IdleSettings | null =>
    parseIdleSettings(
      process.env.IDLE_TIMEOUT_SECONDS,
      process.env.IDLE_PREVIEW
    )
  )
  .client((): IdleSettings | null => null)
