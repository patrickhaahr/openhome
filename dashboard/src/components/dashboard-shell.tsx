import { getRouteApi, Link, Outlet, useRouter } from "@tanstack/react-router"
import { useEffect, useState } from "react"
import { PlugIcon } from "@phosphor-icons/react/dist/csr/Plug"
import {
  DaylightProvider,
  Horizon,
  ThemeToggle,
  useDaylight,
} from "@/components/daylight"
import { CurrencyProvider } from "@/components/currency"
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
import {
  barClock,
  iconLink,
  LogoDot,
  TabGlyphs,
  topTab,
  topTabLabel,
  wordmark,
} from "@/components/nav-parts"
import { Rail, TabBar } from "@/components/side-nav"
import { features } from "@/lib/features"
import { defaultIdleSettings } from "@/lib/idle"

const rootRoute = getRouteApi("__root__")

export function DashboardShell(): React.JSX.Element {
  const router = useRouter()
  const { openedAt, nav, currency, idle } = rootRoute.useLoaderData()
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
        <CurrencyProvider initial={currency}>
          <IdleProvider
            settings={idleSettings}
            overlay={
              <IdleDates lead={(dates) => <IdleMilestone dates={dates} />} />
            }
          >
            <Shell />
          </IdleProvider>
        </CurrencyProvider>
      </NavLayoutProvider>
    </DaylightProvider>
  )
}

const page =
  "mx-auto w-full max-w-360 px-margin pt-page-block pb-16 in-data-[nav=side]:col-2 in-data-[nav=side]:row-1 max-phone:col-1 max-phone:row-1 max-phone:pb-tabbar-room phone:has-[>:is([data-slot=home],[data-slot=wx],[data-slot=controls],[data-slot=today])]:pb-page-block has-[>:is([data-slot=lists],[data-slot=binder])]:pt-page-block-snug tablet:has-[[data-slot=binder]]:pb-4 binder-wide:has-[[data-slot=binder]]:max-w-none binder-wide:has-[[data-slot=binder]]:pb-6 has-[[data-slot=photo-wall]]:max-w-none has-[[data-slot=photo-wall]]:px-0 phone:has-[[data-slot=photo-wall]]:pb-0 has-[[data-slot=travel]]:max-w-none has-[[data-slot=travel]]:p-0"

function Shell(): React.JSX.Element {
  const { layout, labels } = useNavLayout()
  const asleep = useIdlePhase() !== "awake"

  return (
    <div
      data-slot="shell"
      className="group/shell grid min-h-screen grid-cols-1 grid-rows-shell shell-bar data-[nav=side]:grid-cols-rail data-[nav=side]:grid-rows-1 supports-[height:1svh]:min-h-svh max-phone:grid-cols-1 max-phone:grid-rows-1"
      data-nav={layout}
      data-labels={labels}
      inert={asleep}
      aria-hidden={asleep ? true : undefined}
    >
      <a
        className="fixed top-3 left-margin z-60 -translate-y-skip-hidden rounded-full bg-ink px-5 py-3 font-semibold text-ground focus:translate-y-0"
        href="#page-content"
      >
        Skip to page content
      </a>
      <p id={logoHintId} hidden>
        Double-tap to switch between the top bar and the sidebar.
      </p>
      {layout === "side" ? <Rail /> : <TopBar />}
      <TabBar />
      <main id="page-content" tabIndex={-1} data-slot="page" className={page}>
        <Outlet />
      </main>
    </div>
  )
}

function TopBar(): React.JSX.Element {
  const onLogoClick = useLogoTaps()

  return (
    <header
      data-slot="topbar"
      className="sticky top-0 z-40 grid h-bar grid-cols-flanked items-center gap-topbar-gap bg-ground px-margin max-tablet:h-auto max-tablet:grid-cols-pair max-tablet:grid-rows-bar-stack max-tablet:gap-y-0 max-phone:hidden"
    >
      <Link
        to="/"
        className={wordmark}
        aria-label="OpenHome, Home"
        aria-describedby={logoHintId}
        onClick={onLogoClick}
      >
        openhome
        <LogoDot />
      </Link>
      <nav
        data-slot="topnav"
        className="min-w-0 overflow-x-auto fade-end max-tablet:col-span-full max-tablet:row-2 max-tablet:-mx-margin max-tablet:px-nav-bleed"
        aria-label="Main navigation"
      >
        <ul className="flex w-max pr-8">
          {features.map((feature) => (
            <li key={feature.id}>
              <FeatureLink
                feature={feature}
                className={topTab}
                title={feature.label}
              >
                <TabGlyphs Icon={feature.icon} size={20} glyph="inline" />
                <span className={topTabLabel}>{feature.label}</span>
              </FeatureLink>
            </li>
          ))}
        </ul>
      </nav>
      <div className="flex items-center gap-1 max-tablet:justify-self-end">
        <TopbarClock />
        <Link
          to="/connection"
          className={iconLink}
          aria-label="Connection"
          title="Connection"
        >
          <PlugIcon size={20} aria-hidden="true" />
        </Link>
        <ThemeToggle />
      </div>
      <Horizon placement="bar" />
    </header>
  )
}

function TopbarClock(): React.JSX.Element {
  const { now } = useDaylight()

  return (
    <time className={barClock} dateTime={now.toISOString()}>
      {timeFormat.format(now)}
    </time>
  )
}
