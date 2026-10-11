import { createFileRoute } from "@tanstack/react-router"
import { LightControls } from "@/components/light-controls"
import { SpeakerRemote } from "@/components/speaker-remote"
import { TvRemote } from "@/components/tv-remote"
import { loadControls } from "@/lib/controls-loader"
import { features } from "@/lib/features"
import { sendLightCommand } from "@/lib/lights.functions"
import { sendSpeakerCommand, sendTvCommand } from "@/lib/ir.functions"

const controls = features[1] satisfies { id: "controls" }

/** The focused Controls view: every home control, including the full TV and speaker remotes. */
export const Route = createFileRoute("/controls")({
  head: () => ({ meta: [{ title: `${controls.title} · OpenHome` }] }),
  loader: loadControls,
  headers: () => ({ "Cache-Control": "no-store" }),
  component: ControlsPage,
})

function ControlsPage(): React.JSX.Element {
  const { health, tvRemote, speakerRemote } = Route.useLoaderData()

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
        <div className="controls-stack">
          <LightControls
            layout="full"
            connectionState={health.state}
            sendCommand={sendLightCommand}
          />
          <SpeakerRemote
            layout="full"
            connectionState={health.state}
            remote={speakerRemote}
            sendCommand={sendSpeakerCommand}
          />
        </div>
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
