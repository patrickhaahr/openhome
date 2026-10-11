import { AtomIcon } from "@phosphor-icons/react/dist/csr/Atom"
import { BarbellIcon } from "@phosphor-icons/react/dist/csr/Barbell"
import { CloudSunIcon } from "@phosphor-icons/react/dist/csr/CloudSun"
import { FilmSlateIcon } from "@phosphor-icons/react/dist/csr/FilmSlate"
import { HeartIcon } from "@phosphor-icons/react/dist/csr/Heart"
import { ImagesIcon } from "@phosphor-icons/react/dist/csr/Images"
import { PathIcon } from "@phosphor-icons/react/dist/csr/Path"
import { SlidersHorizontalIcon } from "@phosphor-icons/react/dist/csr/SlidersHorizontal"
import { HardDrivesIcon } from "@phosphor-icons/react/dist/csr/HardDrives"

/** Destinations delivered by later release slices, not claims of provider health. */
export const features = [
  {
    id: "weather",
    label: "Weather",
    title: "Weather",
    icon: CloudSunIcon,
    description: "Current conditions and forecasts for time outside.",
    unavailable:
      "Weather has not been added to this dashboard. No current conditions or forecast are available.",
  },
  {
    id: "controls",
    label: "Controls",
    title: "Home controls",
    icon: SlidersHorizontalIcon,
    description: "Lights, TV and speaker, together in one place.",
    unavailable:
      "Light commands are available on Home. TV and speaker remotes have not been added to this dashboard. Observed device states are unavailable.",
  },
  {
    id: "travel",
    label: "Travel",
    title: "Routes & transit",
    icon: PathIcon,
    description: "Walking routes, departures and journeys.",
    unavailable:
      "Routes and transit have not been added to this dashboard. No route, departures or live vehicle positions are available.",
  },
  {
    id: "workout",
    label: "Workout",
    title: "Workout",
    icon: BarbellIcon,
    description: "Training, recovery and exercise progress.",
    unavailable:
      "Workout features have not been added to this dashboard. Training records, recovery and progress are unavailable.",
  },
  {
    id: "wishlist",
    label: "Wishlist",
    title: "Wishlist",
    icon: HeartIcon,
    description: "Everyone’s wishlists, in a shared household view.",
    unavailable:
      "Wishlists have not been added to this dashboard. Household users and their lists are unavailable.",
  },
  {
    id: "watchlist",
    label: "Watchlist",
    title: "Watchlist",
    icon: FilmSlateIcon,
    description: "Movies and series, with everyone’s lists together.",
    unavailable:
      "Watchlists have not been added to this dashboard. Household users and their viewing lists are unavailable.",
  },
  {
    id: "photos",
    label: "Photos",
    title: "Photos",
    icon: ImagesIcon,
    description: "Household photos and a quiet idle slideshow.",
    unavailable:
      "Photos have not been added to this dashboard. No photo library is connected here, and the idle slideshow is unavailable.",
  },
  {
    id: "server",
    label: "Server",
    title: "Server",
    icon: HardDrivesIcon,
    description: "AdGuard protection, containers and feeds.",
    unavailable:
      "Server features have not been added to this dashboard. Protection, container health and feed information are unavailable.",
  },
  {
    id: "agent",
    label: "Agent",
    title: "Agent",
    icon: AtomIcon,
    description: "A conversation with your home, when you need it.",
    unavailable:
      "Agent conversations and voice have not been added to this dashboard. The microphone is not active.",
  },
] as const

/** One destination's display metadata, independent of provider configuration or health. */
export type Feature = (typeof features)[number]
