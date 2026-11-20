import { getConnectionHealth } from "./connection.functions"
import type { IrRemotes } from "./ir"
import { loadIrRemotes } from "./ir.functions"

export type ControlsData = {
  health: Awaited<ReturnType<typeof getConnectionHealth>>
  tvRemote: Promise<IrRemotes["tv"]>
  speakerRemote: Promise<IrRemotes["speaker"]>
}

export async function loadControls(): Promise<ControlsData> {
  const remotes = loadIrRemotes()

  return {
    tvRemote: remotes.then(({ tv }) => tv),
    speakerRemote: remotes.then(({ speaker }) => speaker),
    health: await getConnectionHealth(),
  }
}
