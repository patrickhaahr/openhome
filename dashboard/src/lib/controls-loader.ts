import { getConnectionHealth } from "./connection.functions"
import { loadTvRemote } from "./tv.functions"

/** What the home controls render: connection health now, TV discovery when it arrives. */
export type ControlsData = {
  health: Awaited<ReturnType<typeof getConnectionHealth>>
  tvRemote: ReturnType<typeof loadTvRemote>
}

/**
 * Load what home controls need: the connection check now, TV discovery deferred.
 *
 * @returns Connection health and a promise of the discovered TV remote, so a slow IR bridge never delays the page.
 */
export async function loadControls(): Promise<ControlsData> {
  return { tvRemote: loadTvRemote(), health: await getConnectionHealth() }
}
