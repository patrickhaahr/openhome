import { getConnectionHealth } from "./connection.functions"
import type { IrRemotes } from "./ir"
import { loadIrRemotes } from "./ir.functions"

/** What the home controls render: connection health now, IR Remote discovery when it arrives. */
export type ControlsData = {
  health: Awaited<ReturnType<typeof getConnectionHealth>>
  tvRemote: Promise<IrRemotes["tv"]>
  speakerRemote: Promise<IrRemotes["speaker"]>
}

/**
 * Load what home controls need: the connection check now, IR Remote discovery deferred.
 *
 * @returns Connection health and promises of each discovered IR Remote, so a slow IR bridge never delays the page.
 */
export async function loadControls(): Promise<ControlsData> {
  // One discovery serves both remotes; each card awaits only its own commands.
  const remotes = loadIrRemotes()

  return {
    tvRemote: remotes.then(({ tv }) => tv),
    speakerRemote: remotes.then(({ speaker }) => speaker),
    health: await getConnectionHealth(),
  }
}
