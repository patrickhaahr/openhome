import { Suspense } from "react"
import {
  Await,
  createFileRoute,
  getRouteApi,
  Link,
} from "@tanstack/react-router"
import { ArrowRightIcon } from "@phosphor-icons/react/dist/csr/ArrowRight"
import { HeartIcon } from "@phosphor-icons/react/dist/csr/Heart"
import { PathIcon } from "@phosphor-icons/react/dist/csr/Path"
import { WarningCircleIcon } from "@phosphor-icons/react/dist/csr/WarningCircle"
import { SunDay } from "@/components/daylight"
import { WeatherNow } from "@/components/weather-overview"
import { LightControls } from "@/components/light-controls"
import { LiveClock } from "@/components/live-clock"
import { SpeakerRemote } from "@/components/speaker-remote"
import { TvRemote } from "@/components/tv-remote"
import { loadControls } from "@/lib/controls-loader"
import { loadGlance, type Glance } from "@/lib/glance"
import { sendLightCommand } from "@/lib/lights.functions"
import { sendSpeakerCommand, sendTvCommand } from "@/lib/ir.functions"
import { photoUrl } from "@/lib/photos"
import { posterUrl } from "@/lib/watchlist"
import { getWeather } from "@/lib/weather.functions"
import { unavailableWeather } from "@/lib/weather"

export const Route = createFileRoute("/")({
  head: () => ({ meta: [{ title: "Home · OpenHome" }] }),
  loader: async () => {
    const weather = getWeather().catch(() => unavailableWeather)
    const glance = loadGlance(new Date())

    return { ...(await loadControls()), weather, glance }
  },
  headers: () => ({ "Cache-Control": "no-store" }),
  component: HomePage,
  errorComponent: HomeError,
})

const rootRoute = getRouteApi("__root__")

const count = new Intl.NumberFormat("da-DK")

const kilometers = new Intl.NumberFormat("da-DK", {
  minimumFractionDigits: 1,
  maximumFractionDigits: 1,
})

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
      <section className="directory" aria-label="Home features">
        <div className="directory-group">
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
        </div>
        <div className="directory-group">
          <Glances glance={glance} />
        </div>
      </section>
    </div>
  )
}

/** Navigable rows whose live value arrives on its own; a failed read says so instead of "0". */
function Glances({
  glance,
}: {
  glance: Glance | undefined
}): React.JSX.Element {
  return (
    <>
      <DirectoryLink to="/workout" name="Workout">
        <Deferred promise={glance?.workout}>
          {(workout) => {
            const sessions = workout.weeks.reduce((sum, week) => sum + week, 0)

            return (
              <>
                <WeekBars weeks={workout.weeks} />
                <Count
                  value={sessions}
                  label={
                    workout.last === null
                      ? "No sessions yet"
                      : `${sessions === 1 ? "session" : "sessions"} in 8 weeks`
                  }
                />
              </>
            )
          }}
        </Deferred>
      </DirectoryLink>
      <DirectoryLink to="/wishlist" name="Wishlist">
        <Deferred promise={glance?.wishlist}>
          {(wishlist) => (
            <>
              <HeartIcon
                className="dir-glyph"
                size={22}
                weight="fill"
                aria-hidden="true"
              />
              <Count
                value={wishlist.wishes}
                label={`${wishlist.wishes === 1 ? "wish" : "wishes"} from ${wishlist.people} ${wishlist.people === 1 ? "person" : "people"}`}
              />
            </>
          )}
        </Deferred>
      </DirectoryLink>
      <DirectoryLink to="/watchlist" name="Watchlist">
        <Deferred promise={glance?.watchlist}>
          {(watchlist) => (
            <>
              <span className="dir-posters" aria-hidden="true">
                {watchlist.posters.map((poster) => (
                  <span
                    key={poster.id}
                    className="dir-poster"
                    // SAFETY: CSSProperties omits custom properties; --hue is a degree value read by .dir-poster.
                    style={{ "--hue": poster.hue } as React.CSSProperties}
                  >
                    {poster.poster === null ? (
                      poster.title.charAt(0)
                    ) : (
                      <img
                        src={posterUrl(poster.poster)}
                        alt=""
                        loading="lazy"
                      />
                    )}
                  </span>
                ))}
              </span>
              <Count
                value={watchlist.planned + watchlist.watching}
                label={
                  watchlist.next === null
                    ? "to watch"
                    : `to watch, next ${watchlist.next}`
                }
              />
            </>
          )}
        </Deferred>
      </DirectoryLink>
      <DirectoryLink to="/photos" name="Photos">
        <Deferred promise={glance?.photos}>
          {(photos) => (
            <>
              <span className="dir-thumbs" aria-hidden="true">
                {photos.covers.slice(0, 3).map((cover) => (
                  <img
                    key={cover.album}
                    className="dir-thumb"
                    src={photoUrl(cover.album, cover.asset, "thumbnail")}
                    alt=""
                    loading="lazy"
                  />
                ))}
              </span>
              <Count
                value={photos.photos}
                label={
                  photos.covers.length === 0 ? "photos, no albums" : "photos"
                }
              />
            </>
          )}
        </Deferred>
      </DirectoryLink>
      <DirectoryLink to="/travel" name="Travel">
        {glance === undefined ? (
          <RouteTrace path={null} />
        ) : (
          <Suspense fallback={<RouteTrace path={null} />}>
            <Await promise={glance.travel}>
              {(travel) => (
                <>
                  <RouteTrace path={travel?.path ?? null} />
                  {travel?.meters === null || travel?.meters === undefined ? (
                    <span className="sr-only">Plan a loop from home</span>
                  ) : (
                    <span className="dir-count">
                      {kilometers.format(travel.meters / 1000)}
                      <span className="dir-unit"> km</span>
                      <span className="sr-only"> active route</span>
                    </span>
                  )}
                </>
              )}
            </Await>
          </Suspense>
        )}
      </DirectoryLink>
    </>
  )
}

/** A figure for the eye; its meaning for assistive technology. */
function Count({
  value,
  label,
}: {
  value: number
  label: string
}): React.JSX.Element {
  return (
    <span className="dir-count">
      {count.format(value)}
      <span className="sr-only"> {label}</span>
    </span>
  )
}

/** The active loop's outline, or the path glyph when none is selected. */
function RouteTrace({ path }: { path: string | null }): React.JSX.Element {
  if (path === null)
    return <PathIcon className="dir-glyph" size={30} aria-hidden="true" />

  return (
    <svg className="dir-route" viewBox="-4 -4 108 108" aria-hidden="true">
      <path d={path} />
    </svg>
  )
}

function DirectoryLink({
  to,
  name,
  children,
}: {
  to: "/workout" | "/wishlist" | "/watchlist" | "/photos" | "/travel"
  name: string
  children: React.ReactNode
}): React.JSX.Element {
  return (
    <Link to={to} className="dir-row">
      <span className="dir-name">{name}</span>
      <span className="dir-value">{children}</span>
      <span className="dir-go" aria-hidden="true">
        <ArrowRightIcon size={18} />
      </span>
    </Link>
  )
}

function Deferred<T>({
  promise,
  children,
}: {
  promise: Promise<T | null> | undefined
  children: (value: T) => React.ReactNode
}): React.JSX.Element {
  if (promise === undefined) return <Unavailable />

  return (
    <Suspense
      fallback={
        <span className="dir-loading" aria-hidden="true">
          <span />
          <span />
          <span />
        </span>
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

/** Eight weeks of sessions as small columns; the current week is the sun. */
function WeekBars({ weeks }: { weeks: readonly number[] }): React.JSX.Element {
  const peak = Math.max(1, ...weeks)

  return (
    <span className="week-bars" aria-hidden="true">
      {weeks.map((week, index) => (
        <span
          // Weeks are positional: the key is the week's age.
          key={`week-${weeks.length - index}`}
          // SAFETY: CSSProperties omits custom properties; --share is a unitless 0..1 column height read by .week-bars.
          style={
            { "--share": Math.max(0.08, week / peak) } as React.CSSProperties
          }
          data-empty={week === 0}
        />
      ))}
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
