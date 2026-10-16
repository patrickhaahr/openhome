import { getRouteApi, Link, Outlet, useRouter } from "@tanstack/react-router"
import { useEffect } from "react"
import { PlugIcon } from "@phosphor-icons/react/dist/csr/Plug"
import {
  DaylightProvider,
  Horizon,
  ThemeToggle,
  useDaylight,
} from "@/components/daylight"
import { timeFormat } from "@/components/live-clock"
import { features, type Feature } from "@/lib/features"

const rootRoute = getRouteApi("__root__")

/** A top bar on the horizon line, one content landmark, and focus after rendered navigation. */
export function DashboardShell(): React.JSX.Element {
  const router = useRouter()
  const { openedAt } = rootRoute.useLoaderData()

  useEffect(
    () =>
      router.subscribe("onRendered", ({ pathChanged }) => {
        if (pathChanged)
          document
            .getElementById("page-heading")
            ?.focus({ preventScroll: true })
      }),
    [router]
  )

  return (
    <DaylightProvider initialTime={openedAt}>
      <div className="shell">
        <a className="skip-link" href="#page-content">
          Skip to page content
        </a>
        <header className="topbar">
          <Link to="/" className="wordmark" aria-label="OpenHome, Home">
            openhome
            <i aria-hidden="true" />
          </Link>
          <nav className="topnav" aria-label="Main navigation">
            <ul>
              {features.map((feature) => (
                <li key={feature.id}>
                  <FeatureLink feature={feature} />
                </li>
              ))}
            </ul>
          </nav>
          <div className="topbar-end">
            <TopbarClock />
            <Link
              to="/connection"
              className="icon-link"
              aria-label="Connection"
              title="Connection"
            >
              <PlugIcon size={20} aria-hidden="true" />
            </Link>
            <ThemeToggle />
          </div>
          <Horizon />
        </header>
        <main id="page-content" tabIndex={-1} className="page">
          <Outlet />
        </main>
      </div>
    </DaylightProvider>
  )
}

function TopbarClock(): React.JSX.Element {
  const { now } = useDaylight()

  return (
    <time className="topbar-clock" dateTime={now.toISOString()}>
      {timeFormat.format(now)}
    </time>
  )
}

function FeatureLink({ feature }: { feature: Feature }): React.JSX.Element {
  const to = implementedRoute(feature.id)

  // Implemented features use their own typed routes; the rest share the placeholder.
  return to === undefined ? (
    <Link to="/$feature" params={{ feature: feature.id }} data-soon="">
      {feature.label}
    </Link>
  ) : (
    <Link to={to}>{feature.label}</Link>
  )
}

export function implementedRoute(
  id: Feature["id"]
):
  | "/controls"
  | "/weather"
  | "/travel"
  | "/wishlist"
  | "/watchlist"
  | "/photos"
  | "/workout"
  | undefined {
  switch (id) {
    case "controls":
      return "/controls"
    case "weather":
      return "/weather"
    case "travel":
      return "/travel"
    case "wishlist":
      return "/wishlist"
    case "watchlist":
      return "/watchlist"
    case "photos":
      return "/photos"
    case "workout":
      return "/workout"
    case "agent":
    case "server":
      return undefined
  }
}
