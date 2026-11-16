import { Dialog } from "@base-ui/react/dialog"
import { Link, useLocation } from "@tanstack/react-router"
import { useState } from "react"
import { DotsThreeIcon } from "@phosphor-icons/react/dist/csr/DotsThree"
import { HouseIcon } from "@phosphor-icons/react/dist/csr/House"
import { PlugIcon } from "@phosphor-icons/react/dist/csr/Plug"
import { XIcon } from "@phosphor-icons/react/dist/csr/X"
import { Horizon, ThemeToggle, useDaylight } from "@/components/daylight"
import { timeFormat } from "@/components/live-clock"
import { FeatureLink } from "@/components/feature-link"
import { useIdleBlock } from "@/components/idle-block"
import { logoHintId, useLogoTaps } from "@/components/nav-layout"
import {
  barClock,
  iconLink,
  LogoDot,
  railMark,
  TabFace,
  tabLink,
} from "@/components/nav-parts"
import { Button } from "@/components/ui/button"
import { features, type Feature } from "@/lib/features"

/** The phone's bar keeps four destinations; the rest wait behind "More". */
const barIds = [
  "weather",
  "controls",
  "lists",
] as const satisfies ReadonlyArray<Feature["id"]>

const isBarFeature = (feature: Feature): boolean =>
  barIds.some((id) => id === feature.id)

/**
 * The sidebar of tabs from 641 px up: a narrow rail of glyphs (their words stay for
 * assistive technology) whose inner edge is the horizon stood on end.
 */
export function Rail(): React.JSX.Element {
  const onLogoClick = useLogoTaps()

  return (
    <header
      data-slot="rail"
      className="sticky top-0 z-40 col-1 row-1 flex h-screen flex-col items-center bg-ground py-3 supports-[height:1svh]:h-svh max-phone:hidden"
    >
      <Link
        to="/"
        className={railMark}
        aria-label="OpenHome, Home"
        aria-describedby={logoHintId}
        onClick={onLogoClick}
      >
        <span aria-hidden="true">oh</span>
        <LogoDot />
      </Link>
      <nav
        data-slot="rail-nav"
        className="min-h-0 w-full flex-auto overflow-y-auto"
        aria-label="Main navigation"
      >
        <ul className="flex flex-col items-center gap-0.5 py-3">
          {features.map((feature) => (
            <li key={feature.id}>
              <FeatureLink
                feature={feature}
                className={tabLink("rail")}
                title={feature.label}
              >
                <TabFace
                  Icon={feature.icon}
                  label={feature.label}
                  face="rail"
                />
              </FeatureLink>
            </li>
          ))}
        </ul>
      </nav>
      <div className="flex flex-none flex-col items-center gap-1">
        <RailClock />
        <div className="flex flex-col">
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
      </div>
      <Horizon placement="rail" />
    </header>
  )
}

function RailClock(): React.JSX.Element {
  const { now } = useDaylight()

  return (
    <time className={barClock} dateTime={now.toISOString()}>
      {timeFormat.format(now)}
    </time>
  )
}

/** The phone's navigation in either layout: four destinations and "More" along the bottom. */
export function TabBar(): React.JSX.Element {
  const [open, setOpen] = useState(false)
  useIdleBlock(open)
  const { pathname } = useLocation()
  const rest = features.filter((feature) => !isBarFeature(feature))

  const moreActive =
    pathname === "/connection" ||
    rest.some(
      (feature) =>
        pathname === `/${feature.id}` || pathname.startsWith(`/${feature.id}/`)
    )

  return (
    <nav
      data-slot="tabbar"
      className="hidden max-phone:fixed max-phone:inset-x-0 max-phone:bottom-0 max-phone:z-40 max-phone:block max-phone:h-bar max-phone:bg-ground max-phone:pb-safe-bottom"
      aria-label="Main navigation"
    >
      <Horizon placement="tabbar" />
      <ul className="grid h-full grid-cols-5 px-1">
        <li>
          <Link
            to="/"
            className={tabLink("bar")}
            activeOptions={{ exact: true, includeSearch: false }}
          >
            <TabFace Icon={HouseIcon} label="Home" face="bar" />
          </Link>
        </li>
        {features.filter(isBarFeature).map((feature) => (
          <li key={feature.id}>
            <FeatureLink feature={feature} className={tabLink("bar")}>
              <TabFace Icon={feature.icon} label={feature.label} face="bar" />
            </FeatureLink>
          </li>
        ))}
        <li>
          <Dialog.Root open={open} onOpenChange={setOpen}>
            <Dialog.Trigger
              className={tabLink("bar")}
              data-active={moreActive ? "true" : undefined}
            >
              <TabFace Icon={DotsThreeIcon} label="More" face="bar" />
            </Dialog.Trigger>
            <Dialog.Portal>
              <Dialog.Backdrop className="sheet-scrim" />
              {/* Navigation moves focus to the page heading, so closing must not reclaim it. */}
              <Dialog.Popup
                className="fixed inset-x-0 bottom-0 z-51 grid gap-4 rounded-t-sheet bg-plate px-5 pt-4 pb-sheet-bottom text-ink transition-transform duration-mid ease-app outline-none data-ending-style:translate-y-full data-starting-style:translate-y-full"
                finalFocus={false}
              >
                <div className="flex items-center gap-2">
                  <Dialog.Title className="sr-only">More pages</Dialog.Title>
                  <MoreClock />
                  <ThemeToggle />
                  <Dialog.Close
                    render={<Button variant="outline" size="key-icon" />}
                  >
                    <XIcon size={20} aria-hidden="true" />
                    <span className="sr-only">Close</span>
                  </Dialog.Close>
                </div>
                <ul className="grid grid-cols-4 gap-x-1 gap-y-2">
                  {rest.map((feature) => (
                    <li key={feature.id}>
                      <FeatureLink
                        feature={feature}
                        className={tabLink("tile")}
                        onClick={() => setOpen(false)}
                      >
                        <TabFace
                          Icon={feature.icon}
                          label={feature.label}
                          face="tile"
                        />
                      </FeatureLink>
                    </li>
                  ))}
                  <li>
                    <Link
                      to="/connection"
                      className={tabLink("tile")}
                      onClick={() => setOpen(false)}
                    >
                      <TabFace Icon={PlugIcon} label="Connection" face="tile" />
                    </Link>
                  </li>
                </ul>
              </Dialog.Popup>
            </Dialog.Portal>
          </Dialog.Root>
        </li>
      </ul>
    </nav>
  )
}

function MoreClock(): React.JSX.Element {
  const { now } = useDaylight()

  return (
    <time
      className="mr-auto text-figure-md font-medium tracking-figure tabular-nums"
      dateTime={now.toISOString()}
    >
      {timeFormat.format(now)}
    </time>
  )
}
