import { getRouteApi, Link, Outlet, useRouter } from "@tanstack/react-router"
import { useEffect, useState } from "react"
import { PlugIcon } from "@phosphor-icons/react/dist/csr/Plug"
import {
  DaylightProvider,
  Horizon,
  ThemeToggle,
  useDaylight,
} from "@/components/daylight"
import { FeatureLink } from "@/components/feature-link"
import { IdleDates } from "@/components/idle-dates"
import { IdleMilestone } from "@/components/idle-milestone"
import { IdleProvider } from "@/components/idle"
import { useIdlePhase } from "@/components/idle-block"
import { timeFormat } from "@/components/live-clock"
import {
  logoHintId,
  NavLayoutProvider,
  useLogoTaps,
  useNavLayout,
} from "@/components/nav-layout"
import { Rail, TabBar, TabGlyphs } from "@/components/side-nav"
import { features } from "@/lib/features"
import { defaultIdleSettings } from "@/lib/idle"

const rootRoute = getRouteApi("__root__")

/** A top bar on the horizon line, one content landmark, and focus after rendered navigation. */
export function DashboardShell(): React.JSX.Element {
  const router = useRouter()
  const { openedAt, nav, idle } = rootRoute.useLoaderData()
  // The server's settings arrive with the first page; client navigations carry none.
  const [idleSettings] = useState(() => idle ?? defaultIdleSettings)

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
      <NavLayoutProvider initial={nav}>
        <IdleProvider
          settings={idleSettings}
          overlay={
            <IdleDates lead={(dates) => <IdleMilestone dates={dates} />} />
          }
        >
          <Shell />
        </IdleProvider>
      </NavLayoutProvider>
    </DaylightProvider>
  )
}

/** The chosen layout from 641 px up; a phone always gets the bottom tab bar. */
function Shell(): React.JSX.Element {
  const { layout, labels } = useNavLayout()
  // Under the Idle Slideshow the page stays mounted, exactly as it was, but out of reach.
  const asleep = useIdlePhase() !== "awake"

  return (
    <div
      className="shell"
      data-nav={layout}
      data-labels={labels}
      inert={asleep}
      aria-hidden={asleep ? true : undefined}
    >
      <a className="skip-link" href="#page-content">
        Skip to page content
      </a>
      <p id={logoHintId} hidden>
        Double-tap to switch between the top bar and the sidebar.
      </p>
      {layout === "side" ? <Rail /> : <TopBar />}
      <TabBar />
      <main id="page-content" tabIndex={-1} className="page">
        <Outlet />
      </main>
    </div>
  )
}

function TopBar(): React.JSX.Element {
  const onLogoClick = useLogoTaps()

  return (
    <header className="topbar">
      <Link
        to="/"
        className="wordmark"
        aria-label="OpenHome, Home"
        aria-describedby={logoHintId}
        onClick={onLogoClick}
      >
        openhome
        <i aria-hidden="true" />
      </Link>
      <nav className="topnav" aria-label="Main navigation">
        <ul>
          {features.map((feature) => (
            <li key={feature.id}>
              <FeatureLink feature={feature} title={feature.label}>
                <TabGlyphs Icon={feature.icon} size={20} />
                <span className="topnav-label">{feature.label}</span>
              </FeatureLink>
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
