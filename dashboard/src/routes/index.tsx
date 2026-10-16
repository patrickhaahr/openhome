import { Suspense } from "react"
import {
  Await,
  createFileRoute,
  getRouteApi,
  Link,
} from "@tanstack/react-router"
import { ArrowRightIcon } from "@phosphor-icons/react/dist/csr/ArrowRight"
import { HeartbeatIcon } from "@phosphor-icons/react/dist/csr/Heartbeat"
import { LightningIcon } from "@phosphor-icons/react/dist/csr/Lightning"
import { MoonIcon } from "@phosphor-icons/react/dist/csr/Moon"
import { WarningCircleIcon } from "@phosphor-icons/react/dist/csr/WarningCircle"
import { SunDay } from "@/components/daylight"
import { WeatherNow } from "@/components/weather-overview"
import { LightControls } from "@/components/light-controls"
import { LiveClock } from "@/components/live-clock"
import { ScoreRing } from "@/components/score-ring"
import { SpeakerRemote } from "@/components/speaker-remote"
import { TvRemote } from "@/components/tv-remote"
import { loadControls } from "@/lib/controls-loader"
import type { ChartedLine } from "@/lib/exercise-progress"
import { loadGlance, type Glance, type ProgressGlance } from "@/lib/glance"
import { sendLightCommand } from "@/lib/lights.functions"
import { sendSpeakerCommand, sendTvCommand } from "@/lib/ir.functions"
import {
  chargeBand,
  clockDuration,
  restBand,
  restReferenceMin,
  ringShare,
  spokenDuration,
} from "@/lib/recovery-score"
import type { RecoveryDay } from "@/lib/training"
import { getWeather } from "@/lib/weather.functions"
import { unavailableWeather } from "@/lib/weather"

export const Route = createFileRoute("/")({
  head: () => ({ meta: [{ title: "Home · OpenHome" }] }),
  loader: async () => {
    const weather = getWeather().catch(() => unavailableWeather)
    const glance = loadGlance()

    return { ...(await loadControls()), weather, glance }
  },
  headers: () => ({ "Cache-Control": "no-store" }),
  component: HomePage,
  errorComponent: HomeError,
})

const rootRoute = getRouteApi("__root__")

function HomePage(): React.JSX.Element {
  const { openedAt } = rootRoute.useLoaderData()

  const { health, tvRemote, speakerRemote, weather, glance } =
    Route.useLoaderData()

  return (
    <div className="home">
      <h1 id="page-heading" tabIndex={-1} className="sr-only">
        Home
      </h1>
      <section className="home-now" aria-label="Today">
        <LiveClock initialTime={openedAt} />
        <SunDay />
        <WeatherNow weather={weather} />
      </section>
      <section className="home-body" aria-label="Training">
        <RecoveryGlance glance={glance} />
        <ProgressGlanceCard glance={glance} />
      </section>
      <section className="deck" aria-label="Controls">
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
        <SpeakerRemote
          layout="quick"
          connectionState={health.state}
          remote={speakerRemote}
          sendCommand={sendSpeakerCommand}
        />
      </section>
    </div>
  )
}

/** Today's three rings on the paper; tapping them opens the day on Workout. */
function RecoveryGlance({
  glance,
}: {
  glance: Glance | undefined
}): React.JSX.Element {
  return (
    <Link to="/workout" className="recovery-glance">
      <h2 className="sr-only">Recovery today</h2>
      <Deferred promise={glance?.recovery} ghosts={3}>
        {(recovery) => <TodayRings data={recovery} />}
      </Deferred>
    </Link>
  )
}

/** Four weeks of strength; tapping it opens Workout's progress view. */
function ProgressGlanceCard({
  glance,
}: {
  glance: Glance | undefined
}): React.JSX.Element {
  return (
    <Link
      to="/workout"
      search={{ view: "progress", period: "4w" }}
      className="progress-card"
    >
      <h2 className="sr-only">Exercise progress, last 4 weeks</h2>
      <Deferred promise={glance?.progress} ghosts={0}>
        {(progress) => <ProgressGlanceChart progress={progress} />}
      </Deferred>
    </Link>
  )
}

/** Charge, Rest and Effort for today; a glyph names each ring, the words stay for AT. */
function TodayRings({ data }: { data: RecoveryDay }): React.JSX.Element {
  const charge = data.derived_scores.recovery.value
  const rest = data.sleep.total.value
  const effort = data.derived_scores.strain.value
  const missing = "Not recorded"

  return (
    <>
      <ScoreRing
        kind="charge"
        icon={LightningIcon}
        share={ringShare(charge, 100)}
        band={chargeBand(charge)}
        figure={charge === null ? null : String(Math.round(charge))}
        label="Charge"
        spoken={charge === null ? missing : `${Math.round(charge)} % recovery`}
      />
      <ScoreRing
        kind="rest"
        icon={MoonIcon}
        share={ringShare(rest, restReferenceMin)}
        band={restBand(rest)}
        figure={rest === null ? null : clockDuration(rest)}
        label="Rest"
        spoken={rest === null ? missing : `${spokenDuration(rest)} asleep`}
      />
      <ScoreRing
        kind="effort"
        icon={HeartbeatIcon}
        share={ringShare(effort, 100)}
        band={effort === null ? "none" : "fair"}
        figure={effort === null ? null : String(Math.round(effort))}
        label="Effort"
        spoken={
          effort === null ? missing : `strain ${Math.round(effort)} of 100`
        }
      />
    </>
  )
}

const chart = { width: 640, height: 280, inset: 10 } as const

const dayMs = 86_400_000

/**
 * Each recent exercise as a line indexed to 100: the featured ones in colour and named in the
 * legend beneath, the rest quiet. Exact values stay on the Progress view.
 */
function ProgressGlanceChart({
  progress,
}: {
  progress: ProgressGlance
}): React.JSX.Element {
  const { from, to, featured, others } = progress
  const lines = [...others, ...featured]

  if (featured.length === 0)
    return (
      <p className="progress-empty">
        <span className="sr-only">No exercise trends in the last 4 weeks</span>
        <span aria-hidden="true">–</span>
      </p>
    )

  const indexes = lines.flatMap((line) => line.samples.map((s) => s.index))
  const low = Math.min(100, ...indexes) - 2
  const high = Math.max(100, ...indexes) + 2
  const start = Date.parse(`${from}T00:00:00Z`)
  const span = Math.max(dayMs, Date.parse(`${to}T00:00:00Z`) - start)
  const width = chart.width - chart.inset * 2
  const height = chart.height - chart.inset * 2

  const x = (day: string) =>
    chart.inset + ((Date.parse(`${day}T00:00:00Z`) - start) / span) * width

  const y = (index: number) =>
    chart.inset + ((high - index) / (high - low)) * height

  const path = (line: ChartedLine) =>
    line.samples
      .map(
        (sample, position) =>
          `${position === 0 ? "M" : "L"}${x(sample.date).toFixed(1)},${y(sample.index).toFixed(1)}`
      )
      .join(" ")

  const spoken = lines
    .map(
      (line) =>
        `${line.exercise.name} ${Math.round(line.samples.at(-1)?.index ?? 100)}`
    )
    .join(", ")

  return (
    <figure className="progress-glance">
      <div className="progress-glance-plot">
        {/* Stretched to the column; strokes keep their width and the dots are HTML, so
            nothing distorts. */}
        <svg
          viewBox={`0 0 ${chart.width} ${chart.height}`}
          preserveAspectRatio="none"
          role="img"
          aria-label={`Strength over the last 4 weeks, indexed to 100 on each exercise's first day: ${spoken}`}
        >
          <line
            className="progress-glance-base"
            x1={chart.inset}
            x2={chart.width - chart.inset}
            y1={y(100)}
            y2={y(100)}
          />
          {others.map((line) => (
            <path
              key={line.exercise.id}
              className="progress-glance-line"
              d={path(line)}
            />
          ))}
          {featured.map((line, position) => (
            <path
              key={line.exercise.id}
              className="progress-glance-featured"
              data-series={position + 1}
              d={path(line)}
            />
          ))}
        </svg>
        {featured.map((line, position) => {
          const last = line.samples.at(-1)

          return last === undefined ? null : (
            <span
              key={line.exercise.id}
              className="progress-glance-end"
              data-series={position + 1}
              aria-hidden="true"
              // SAFETY: CSSProperties omits custom properties; --x and --y are 0..1 shares of the plot.
              style={
                {
                  "--x": x(last.date) / chart.width,
                  "--y": y(last.index) / chart.height,
                } as React.CSSProperties
              }
            />
          )
        })}
      </div>
      <figcaption className="progress-glance-legend" aria-hidden="true">
        {featured.map((line, position) => (
          <span key={line.exercise.id} data-series={position + 1}>
            {line.exercise.name}
          </span>
        ))}
      </figcaption>
    </figure>
  )
}

function Deferred<T>({
  promise,
  ghosts,
  children,
}: {
  promise: Promise<T | null> | undefined
  /** Placeholder rings while the read is in flight; zero draws the waiting dots. */
  ghosts: number
  children: (value: T) => React.ReactNode
}): React.JSX.Element {
  if (promise === undefined) return <Unavailable />

  return (
    <Suspense
      fallback={
        ghosts === 0 ? (
          <span className="dir-loading" aria-hidden="true">
            <span />
            <span />
            <span />
          </span>
        ) : (
          Array.from({ length: ghosts }, (_, ghost) => (
            <span key={ghost} className="recovery-ghost" aria-hidden="true" />
          ))
        )
      }
    >
      <Await promise={promise}>
        {(value) => (value === null ? <Unavailable /> : children(value))}
      </Await>
    </Suspense>
  )
}

/** A read that failed: a quiet mark, never a zero. */
function Unavailable(): React.JSX.Element {
  return (
    <span className="dir-pending" title="Unavailable">
      <WarningCircleIcon size={20} aria-hidden="true" />
      <span className="sr-only">Unavailable</span>
    </span>
  )
}

function HomeError(): React.JSX.Element {
  return (
    <section className="lost">
      <h1 id="page-heading" tabIndex={-1} className="page-title">
        Home connection unavailable
      </h1>
      <p className="lost-text">
        The dashboard could not check OpenHome. Open Connection to try again.
      </p>
      <Link to="/connection" className="action-link">
        Open Connection <ArrowRightIcon size={20} aria-hidden="true" />
      </Link>
    </section>
  )
}
