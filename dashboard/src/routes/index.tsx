import { Suspense } from "react"
import {
  Await,
  createFileRoute,
  getRouteApi,
  Link,
} from "@tanstack/react-router"
import { ArrowRightIcon } from "@phosphor-icons/react/dist/csr/ArrowRight"
import { WarningCircleIcon } from "@phosphor-icons/react/dist/csr/WarningCircle"
import { SunDay } from "@/components/daylight"
import { WeatherNow } from "@/components/weather-now"
import { LightControls } from "@/components/light-controls"
import { LiveClock } from "@/components/live-clock"
import { actionLink, PageTitle } from "@/components/page-tools"
import { SpeakerRemote } from "@/components/speaker-remote"
import { RecoveryRings } from "@/components/training-summaries"
import { TvRemote } from "@/components/tv-remote"
import { loadControls } from "@/lib/controls-loader"
import type { ChartedLine } from "@/lib/exercise-progress"
import { loadGlance, type Glance, type ProgressGlance } from "@/lib/glance"
import { sendLightCommand } from "@/lib/lights.functions"
import { sendSpeakerCommand, sendTvCommand } from "@/lib/ir.functions"
import { getWeather } from "@/lib/weather.functions"
import { percent } from "@/components/weather-figures"
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
    <div
      data-slot="home"
      className="grid min-h-fit-screen grid-cols-home gap-home-gap max-home-narrow:min-h-0 max-home-narrow:grid-cols-trail max-tablet:gap-10 max-phone:grid-cols-1"
    >
      <h1 id="page-heading" tabIndex={-1} className="sr-only">
        Home
      </h1>
      <section
        className="flex flex-col justify-between gap-home-stack max-home-narrow:col-start-1 max-tablet:grid max-tablet:grid-cols-home-now max-tablet:items-end max-tablet:gap-x-8 max-phone:grid-cols-1 max-phone:gap-y-6"
        aria-label="Today"
      >
        <LiveClock initialTime={openedAt} />
        <WeatherNow weather={weather} />
        <SunDay />
      </section>
      <section
        className="grid grid-rows-home-body gap-home-body-gap max-home-narrow:col-start-1"
        aria-label="Training"
      >
        <RecoveryGlance glance={glance} />
        <ProgressGlanceCard glance={glance} />
      </section>
      <section
        className="flex flex-col items-center justify-between gap-3 max-home-narrow:col-start-2 max-home-narrow:row-start-1 max-home-narrow:row-end-3 max-home-narrow:justify-start max-phone:col-start-1 max-phone:row-auto max-phone:flex-row max-phone:flex-wrap max-phone:justify-center"
        aria-label="Controls"
      >
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

/** Today's three rings on the paper; tapping them opens the day on Recovery. */
function RecoveryGlance({
  glance,
}: {
  glance: Glance | undefined
}): React.JSX.Element {
  return (
    <Link
      to="/recovery"
      className="mt-1 flex min-h-36 items-center justify-center gap-home-rings-gap text-inherit no-underline transition-lift duration-mid ease-app hover:-translate-y-0.5 active:scale-99 max-phone:gap-3"
    >
      <h2 className="sr-only">Recovery today</h2>
      <Deferred promise={glance?.recovery} ghosts={3}>
        {(recovery) => (
          <RecoveryRings data={recovery} missing="Not recorded" size="glance" />
        )}
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
      className="group/progress flex min-h-48 text-inherit no-underline active:scale-99"
    >
      <h2 className="sr-only">Exercise progress, last 4 weeks</h2>
      <Deferred promise={glance?.progress} ghosts={0}>
        {(progress) => <ProgressGlanceChart progress={progress} />}
      </Deferred>
    </Link>
  )
}

const trace = {
  fill: "none",
  strokeLinecap: "round",
  strokeLinejoin: "round",
  vectorEffect: "non-scaling-stroke",
} as const

/** The four featured lines: the accent first, then three hues of similar lightness. */
const series = [
  {
    stroke: "stroke-progress-1",
    dot: "bg-progress-1",
    swatch: "before:bg-progress-1",
    draw: "motion-safe:animate-glance-draw-1",
  },
  {
    stroke: "stroke-progress-2",
    dot: "bg-progress-2",
    swatch: "before:bg-progress-2",
    draw: "motion-safe:animate-glance-draw-2",
  },
  {
    stroke: "stroke-progress-3",
    dot: "bg-progress-3",
    swatch: "before:bg-progress-3",
    draw: "motion-safe:animate-glance-draw-3",
  },
  {
    stroke: "stroke-glance-4",
    dot: "bg-glance-4",
    swatch: "before:bg-glance-4",
    draw: "motion-safe:animate-glance-draw-4",
  },
] as const

const tone = (position: number) => series[position] ?? series[0]

/** The waiting dots rise one after the other. */
const waits = [
  "motion-safe:animate-glance-wait-1",
  "motion-safe:animate-glance-wait-2",
  "motion-safe:animate-glance-wait-3",
] as const

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
      <p className="text-lead text-muted-ink">
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
    <figure className="grid flex-1 grid-rows-fill-hug gap-4">
      <div className="relative min-h-40 max-home-narrow:min-h-64">
        {/* Stretched to the column; strokes keep their width and the dots are HTML, so
            nothing distorts. */}
        <svg
          className="absolute inset-0 size-full overflow-visible"
          viewBox={`0 0 ${chart.width} ${chart.height}`}
          preserveAspectRatio="none"
          role="img"
          aria-label={`Strength over the last 4 weeks, indexed to 100 on each exercise's first day: ${spoken}`}
        >
          <line
            {...trace}
            className="stroke-muted-ink"
            strokeWidth={1.5}
            strokeDasharray="2 6"
            x1={chart.inset}
            x2={chart.width - chart.inset}
            y1={y(100)}
            y2={y(100)}
          />
          {others.map((line) => (
            <path
              {...trace}
              key={line.exercise.id}
              className="stroke-ink opacity-12 group-hover/progress:opacity-20"
              strokeWidth={2.5}
              d={path(line)}
            />
          ))}
          {featured.map((line, position) => (
            <path
              {...trace}
              key={line.exercise.id}
              className={`${tone(position).stroke} ${tone(position).draw}`}
              strokeWidth={4}
              d={path(line)}
            />
          ))}
        </svg>
        {featured.map((line, position) => {
          const last = line.samples.at(-1)

          return last === undefined ? null : (
            <span
              key={line.exercise.id}
              className={`absolute top-(--y) left-(--x) -mt-1.75 -ml-1.75 size-3.5 rounded-full border-3 border-ground motion-safe:animate-glance-rise ${tone(position).dot}`}
              aria-hidden="true"
              // SAFETY: CSSProperties omits custom properties; --x and --y are the dot's shares of the plot.
              style={
                {
                  "--x": percent(x(last.date) / chart.width),
                  "--y": percent(y(last.index) / chart.height),
                } as React.CSSProperties
              }
            />
          )
        })}
      </div>
      <figcaption
        className="flex flex-wrap justify-center gap-x-6 gap-y-2 text-control font-medium tracking-nav"
        aria-hidden="true"
      >
        {featured.map((line, position) => (
          <span
            key={line.exercise.id}
            className={`inline-flex items-center gap-2 before:h-1 before:w-3.5 before:rounded-full ${tone(position).swatch}`}
          >
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
          <span className="inline-flex gap-1.5" aria-hidden="true">
            {waits.map((wait) => (
              <span
                key={wait}
                className={`size-2 rounded-full bg-line-strong ${wait}`}
              />
            ))}
          </span>
        ) : (
          Array.from({ length: ghosts }, (_, ghost) => (
            <span
              key={ghost}
              className="aspect-square w-36 rounded-full border-10 border-wash"
              aria-hidden="true"
            />
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
    <span className="inline-flex text-muted-ink" title="Unavailable">
      <WarningCircleIcon size={20} aria-hidden="true" />
      <span className="sr-only">Unavailable</span>
    </span>
  )
}

function HomeError(): React.JSX.Element {
  return (
    <section className="grid min-h-page-fill content-center justify-items-start gap-6">
      <PageTitle>Home connection unavailable</PageTitle>
      <p className="max-w-120 text-lg leading-normal text-muted-ink">
        The dashboard could not check OpenHome. Open Connection to try again.
      </p>
      <Link to="/connection" className={actionLink}>
        Open Connection <ArrowRightIcon size={20} aria-hidden="true" />
      </Link>
    </section>
  )
}
