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

export const Route = createFileRoute("/controls")({
  head: () => ({ meta: [{ title: `${controls.title} · OpenHome` }] }),
  loader: loadControls,
  headers: () => ({ "Cache-Control": "no-store" }),
  component: ControlsPage,
})

function ControlsPage(): React.JSX.Element {
  const { health, tvRemote, speakerRemote } = Route.useLoaderData()

  return (
    <section
      data-slot="controls"
      className="grid min-h-fit-screen grid-cols-controls gap-flow max-tablet:min-h-0 max-tablet:grid-cols-controls-narrow max-phone:grid-cols-1"
    >
      <PageHead title={controls.title} />
      <div className="flex min-w-0 flex-col gap-4">
        <LightControls
          layout="full"
          connectionState={health.state}
          sendCommand={sendLightCommand}
        />
        <Link
          to="/backlight"
          className="size-18 self-center rounded-full bg-led-tv-sync transition-transform duration-fast ease-app hover:scale-105 active:scale-97"
          title="LED"
        >
          <span className="sr-only">LED · TV backlight</span>
        </Link>
      </div>
      <TvRemote
        layout="full"
        className="max-tablet:col-span-2 max-phone:col-span-1"
        connectionState={health.state}
        remote={tvRemote}
        sendCommand={sendTvCommand}
      />
      <SpeakerRemote
        layout="full"
        className="max-tablet:col-start-2 max-tablet:row-start-1 max-phone:col-auto max-phone:row-auto"
        connectionState={health.state}
        remote={speakerRemote}
        sendCommand={sendSpeakerCommand}
      />
    </section>
  )
}
