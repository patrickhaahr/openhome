import { createFileRoute } from "@tanstack/react-router"
import { LightControls } from "@/components/light-controls"
import { TvRemote } from "@/components/tv-remote"
import { getConnectionHealth } from "@/lib/connection.functions"
import { features } from "@/lib/features"
import { sendLightCommand } from "@/lib/lights.functions"
import { loadTvRemote, sendTvCommand } from "@/lib/tv.functions"

const controls = features[1] satisfies { id: "controls" }

/** The focused Controls view: every home control, including the full TV remote. */
export const Route = createFileRoute("/controls")({
  head: () => ({ meta: [{ title: `${controls.title} · OpenHome` }] }),
  // TV discovery is deferred so a slow IR remote never delays the lights.
  loader: async () => ({
    tvRemote: loadTvRemote(),
    health: await getConnectionHealth(),
  }),
  headers: () => ({ "Cache-Control": "no-store" }),
  component: ControlsPage,
})

function ControlsPage(): React.JSX.Element {
  const { health, tvRemote } = Route.useLoaderData()

  return (
    <section className="focused-page controls-page">
      <header>
        <p className="page-eyebrow">OpenHome / {controls.label}</p>
        <h1 id="page-heading" tabIndex={-1}>
          {controls.title}
        </h1>
        <p className="feature-intro">{controls.description}</p>
      </header>
      <div className="controls-grid">
        <LightControls
          layout="full"
          connectionState={health.state}
          sendCommand={sendLightCommand}
        />
        <TvRemote
          layout="full"
          connectionState={health.state}
          remote={tvRemote}
          sendCommand={sendTvCommand}
        />
      </div>
    </section>
  )
}
