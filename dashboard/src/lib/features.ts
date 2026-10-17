import { AtomIcon } from "@phosphor-icons/react/dist/csr/Atom"
import { BarbellIcon } from "@phosphor-icons/react/dist/csr/Barbell"
import { CloudSunIcon } from "@phosphor-icons/react/dist/csr/CloudSun"
import { ListHeartIcon } from "@phosphor-icons/react/dist/csr/ListHeart"
import { ImagesIcon } from "@phosphor-icons/react/dist/csr/Images"
import { MoonIcon } from "@phosphor-icons/react/dist/csr/Moon"
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
      "Home controls are unavailable. Observed device states are not reported.",
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
    description: "Logged training, exercise progress and body weight.",
    unavailable:
      "Workout features have not been added to this dashboard. Training records and progress are unavailable.",
  },
  {
    id: "recovery",
    label: "Recovery",
    title: "Recovery",
    icon: MoonIcon,
    description: "Charge, rest and effort, with the night in order.",
    unavailable:
      "Recovery has not been added to this dashboard. Sleep, recovery scores and strap workouts are unavailable.",
  },
  {
    id: "lists",
    label: "Lists",
    title: "Lists",
    icon: ListHeartIcon,
    description:
      "Wishes, watching and important dates, for everyone in the household.",
    unavailable:
      "Lists are unavailable. Household wishlists, watchlists and dates cannot be read.",
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
