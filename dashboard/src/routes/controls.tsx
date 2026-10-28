import { createFileRoute, Link } from "@tanstack/react-router"
import { PageHead } from "@/components/page-head"
import { LightControls } from "@/components/light-controls"
import { SpeakerRemote } from "@/components/speaker-remote"
import { TvRemote } from "@/components/tv-remote"
import { loadControls } from "@/lib/controls-loader"
import { features } from "@/lib/features"
import { sendLightCommand } from "@/lib/lights.functions"
import { sendSpeakerCommand, sendTvCommand } from "@/lib/ir.functions"

const controls = features[1] satisfies { id: "controls" }

/**
 * The focused Controls view: three plates filling the screen. The lights as a wall switch,
 * the TV remote in the middle, the speaker's narrow remote on the right.
 */
export const Route = createFileRoute("/controls")({
  head: () => ({ meta: [{ title: `${controls.title} · OpenHome` }] }),
  loader: loadControls,
  headers: () => ({ "Cache-Control": "no-store" }),
  component: ControlsPage,
})

function ControlsPage(): React.JSX.Element {
  const { health, tvRemote, speakerRemote } = Route.useLoaderData()

  return (
    <section className="controls">
      <PageHead title={controls.title} />
      <div className="controls-lights">
        <LightControls
          layout="full"
          connectionState={health.state}
          sendCommand={sendLightCommand}
        />
        <Link to="/backlight" className="backlight-link" title="LED">
          <span className="sr-only">LED · TV backlight</span>
        </Link>
      </div>
      <TvRemote
        layout="full"
        connectionState={health.state}
        remote={tvRemote}
        sendCommand={sendTvCommand}
      />
      <SpeakerRemote
        layout="full"
        connectionState={health.state}
        remote={speakerRemote}
        sendCommand={sendSpeakerCommand}
      />
    </section>
  )
}
