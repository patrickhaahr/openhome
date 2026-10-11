import { Fragment } from "react"
import { createFileRoute, getRouteApi, Link } from "@tanstack/react-router"
import { ArrowRightIcon } from "@phosphor-icons/react/dist/csr/ArrowRight"
import { FeatureCard } from "@/components/feature-card"
import { LightControls } from "@/components/light-controls"
import { LiveClock } from "@/components/live-clock"
import { TvRemote } from "@/components/tv-remote"
import { loadControls } from "@/lib/controls-loader"
import { features } from "@/lib/features"
import { sendLightCommand } from "@/lib/lights.functions"
import { sendTvCommand } from "@/lib/tv.functions"

export const Route = createFileRoute("/")({
  head: () => ({ meta: [{ title: "Home · OpenHome" }] }),
  loader: loadControls,
  headers: () => ({ "Cache-Control": "no-store" }),
  component: HomePage,
  errorComponent: HomeError,
})

const rootRoute = getRouteApi("__root__")

function HomePage(): React.JSX.Element {
  const { openedAt } = rootRoute.useLoaderData()
  const { health, tvRemote } = Route.useLoaderData()

  return (
    <div>
      <header className="page-header">
        <h1 id="page-heading" tabIndex={-1} className="page-eyebrow">
          Home
        </h1>
        <Link to="/connection" className="connection-summary">
          <span
            className="connection-dot"
            data-connected={health.state === "connected"}
            aria-hidden="true"
          />
          {health.state === "connected" ? "Home connected" : "Check connection"}
          <ArrowRightIcon size={16} aria-hidden="true" />
        </Link>
      </header>
      <section className="home-hero" aria-label="Today">
        <LiveClock initialTime={openedAt} />
        <div className="home-note">
          <span className="page-eyebrow">At a glance</span>
          <h2>
            One home.
            <br /> A shared view.
          </h2>
          <p>
            Open a feature for its details. Unavailable features stay visible as
            your dashboard grows.
          </p>
        </div>
      </section>
      <section aria-label="Home features" className="home-features">
        {features.slice(0, 3).map((feature) =>
          feature.id === "controls" ? (
            <Fragment key={feature.id}>
              <LightControls
                layout="quick"
                connectionState={health.state}
                sendCommand={sendLightCommand}
              />
              <TvRemote
                layout="quick"
                connectionState={health.state}
                remote={tvRemote}
                sendCommand={sendTvCommand}
              />
            </Fragment>
          ) : (
            <FeatureCard key={feature.id} feature={feature} />
          )
        )}
      </section>
      <div className="home-bottom">
        <p>
          Features are being added. A connected home does not mean every
          integration is available.
        </p>
        <Link
          to="/$feature"
          params={{ feature: "agent" }}
          className="agent-link"
        >
          Open agent <ArrowRightIcon size={20} aria-hidden="true" />
        </Link>
      </div>
    </div>
  )
}

function HomeError(): React.JSX.Element {
  return (
    <section className="focused-page">
      <h1 id="page-heading" tabIndex={-1}>
        Home connection unavailable
      </h1>
      <p>
        The dashboard could not check OpenHome. Open Connection to try again.
      </p>
      <Link to="/connection" className="action-link">
        Open Connection
      </Link>
    </section>
  )
}
