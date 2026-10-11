import { getConnectionHealth } from "./connection.functions"
import type { IrRemoteState } from "./ir"
import { loadIrRemotes } from "./ir.functions"
import type { SpeakerCommand } from "./speaker"
import type { TvCommand } from "./tv"

/** What the home controls render: connection health now, IR Remote discovery when it arrives. */
export type ControlsData = {
  health: Awaited<ReturnType<typeof getConnectionHealth>>
  tvRemote: Promise<IrRemoteState<TvCommand>>
  speakerRemote: Promise<IrRemoteState<SpeakerCommand>>
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
