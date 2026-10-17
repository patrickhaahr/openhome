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
    <header className="rail">
      <Link
        to="/"
        className="rail-mark"
        aria-label="OpenHome, Home"
        aria-describedby={logoHintId}
        onClick={onLogoClick}
      >
        <span aria-hidden="true">oh</span>
        <i aria-hidden="true" />
      </Link>
      <nav className="rail-nav" aria-label="Main navigation">
        <ul>
          {features.map((feature) => (
            <li key={feature.id}>
              <FeatureLink
                feature={feature}
                className="rail-tab"
                title={feature.label}
              >
                <TabFace Icon={feature.icon} label={feature.label} />
              </FeatureLink>
            </li>
          ))}
        </ul>
      </nav>
      <div className="rail-end">
        <RailClock />
        <div className="rail-keys">
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
      </div>
      <Horizon axis="vertical" />
    </header>
  )
}

function RailClock(): React.JSX.Element {
  const { now } = useDaylight()

  return (
    <time className="rail-clock topbar-clock num" dateTime={now.toISOString()}>
      {timeFormat.format(now)}
    </time>
  )
}

/** The outline glyph, and the filled one the active tab swaps in. */
export function TabGlyphs({
  Icon,
  size = 22,
}: {
  Icon: Feature["icon"]
  size?: number
}): React.JSX.Element {
  return (
    <>
      <Icon size={size} className="tab-glyph" aria-hidden="true" />
      <Icon
        size={size}
        weight="fill"
        className="tab-glyph-on"
        aria-hidden="true"
      />
    </>
  )
}

/** The glyph on its capsule, the word beneath; the capsule is the tone-step plate. */
function TabFace({
  Icon,
  label,
}: {
  Icon: Feature["icon"]
  label: string
}): React.JSX.Element {
  return (
    <>
      <span className="tab-plate" aria-hidden="true">
        <TabGlyphs Icon={Icon} />
      </span>
      <span className="tab-label">{label}</span>
    </>
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
    <nav className="tabbar" aria-label="Main navigation">
      <Horizon />
      <ul>
        <li>
          <Link
            to="/"
            className="rail-tab"
            activeOptions={{ exact: true, includeSearch: false }}
          >
            <TabFace Icon={HouseIcon} label="Home" />
          </Link>
        </li>
        {features.filter(isBarFeature).map((feature) => (
          <li key={feature.id}>
            <FeatureLink feature={feature} className="rail-tab">
              <TabFace Icon={feature.icon} label={feature.label} />
            </FeatureLink>
          </li>
        ))}
        <li>
          <Dialog.Root open={open} onOpenChange={setOpen}>
            <Dialog.Trigger
              className="rail-tab"
              data-active={moreActive ? "true" : undefined}
            >
              <TabFace Icon={DotsThreeIcon} label="More" />
            </Dialog.Trigger>
            <Dialog.Portal>
              <Dialog.Backdrop className="sheet-scrim" />
              {/* Navigation moves focus to the page heading, so closing must not reclaim it. */}
              <Dialog.Popup className="more-sheet" finalFocus={false}>
                <div className="more-head">
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
                <ul className="more-grid">
                  {rest.map((feature) => (
                    <li key={feature.id}>
                      <FeatureLink
                        feature={feature}
                        className="more-tile"
                        onClick={() => setOpen(false)}
                      >
                        <TabFace Icon={feature.icon} label={feature.label} />
                      </FeatureLink>
                    </li>
                  ))}
                  <li>
                    <Link
                      to="/connection"
                      className="more-tile"
                      onClick={() => setOpen(false)}
                    >
                      <TabFace Icon={PlugIcon} label="Connection" />
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
    <time className="more-clock num" dateTime={now.toISOString()}>
      {timeFormat.format(now)}
    </time>
  )
}
