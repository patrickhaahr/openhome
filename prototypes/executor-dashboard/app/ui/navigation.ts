import { HouseIcon } from "@phosphor-icons/react/dist/csr/House";
import { PathIcon } from "@phosphor-icons/react/dist/csr/Path";
import { CloudSunIcon } from "@phosphor-icons/react/dist/csr/CloudSun";
import { SlidersHorizontalIcon } from "@phosphor-icons/react/dist/csr/SlidersHorizontal";
import { BarbellIcon } from "@phosphor-icons/react/dist/csr/Barbell";
import { HeartIcon } from "@phosphor-icons/react/dist/csr/Heart";
import { FilmSlateIcon } from "@phosphor-icons/react/dist/csr/FilmSlate";
import { ImagesIcon } from "@phosphor-icons/react/dist/csr/Images";
import { CalendarDotsIcon } from "@phosphor-icons/react/dist/csr/CalendarDots";
import { AtomIcon } from "@phosphor-icons/react/dist/csr/Atom";

/** Dedicated feature destinations in the sidebar prototype. */
export const pages = [
  { id: "home", label: "Home", shortLabel: "Home", icon: HouseIcon },
  { id: "travel", label: "Routes & transit", shortLabel: "Travel", icon: PathIcon },
  { id: "weather", label: "Weather", shortLabel: "Weather", icon: CloudSunIcon },
  { id: "controls", label: "Home controls", shortLabel: "Controls", icon: SlidersHorizontalIcon },
  { id: "workout", label: "Workout", shortLabel: "Workout", icon: BarbellIcon },
  { id: "wishlist", label: "Wishlist", shortLabel: "Wishlist", icon: HeartIcon },
  { id: "watchlist", label: "Watchlist", shortLabel: "Watchlist", icon: FilmSlateIcon },
  { id: "photos", label: "Photos", shortLabel: "Photos", icon: ImagesIcon },
  { id: "calendar", label: "Calendar", shortLabel: "Calendar", icon: CalendarDotsIcon },
  { id: "agent", label: "Agent", shortLabel: "Agent", icon: AtomIcon },
] as const;

/** URL-backed page identity, independent of household selection. */
export type Page = (typeof pages)[number]["id"];

/** Unknown links safely return to Home. */
export function readPage(): Page {
  const value = new URLSearchParams(location.search).get("page");
  return pages.find((page) => page.id === value)?.id ?? "home";
}
