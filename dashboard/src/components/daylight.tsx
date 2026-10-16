import {
  createContext,
  use,
  useEffect,
  useMemo,
  useRef,
  useSyncExternalStore,
} from "react"
import { MoonIcon } from "@phosphor-icons/react/dist/csr/Moon"
import { SunIcon } from "@phosphor-icons/react/dist/csr/Sun"
import { SunHorizonIcon } from "@phosphor-icons/react/dist/csr/SunHorizon"
import { daylight, type Daylight } from "@/lib/daylight"
import { useMinute } from "@/lib/use-minute"

/** "sun" follows the sky; "day" and "night" are a household's explicit choice. */
export type ThemeMode = "sun" | "day" | "night"

const storageKey = "openhome-theme"

type DaylightState = {
  readonly now: Date
  readonly sky: Daylight
  readonly mode: ThemeMode
  readonly setMode: (mode: ThemeMode) => void
}

const DaylightContext = createContext<DaylightState | null>(null)

export function useDaylight(): DaylightState {
  const state = use(DaylightContext)

  if (state === null) throw new Error("useDaylight needs a DaylightProvider")

  return state
}

/**
 * Applies the theme before first paint: the stored choice, otherwise the phase the server
 * saw. The provider takes over after hydration.
 */
export function ThemeScript({
  phase,
}: {
  phase: Daylight["phase"]
}): React.JSX.Element {
  const script = `try{var m=localStorage.getItem(${JSON.stringify(storageKey)});var n=m==="night"||(m!=="day"&&${JSON.stringify(phase)}==="night");document.documentElement.classList.toggle("dark",n)}catch(e){document.documentElement.classList.toggle("dark",${JSON.stringify(phase === "night")})}`

  return <script dangerouslySetInnerHTML={{ __html: script }} />
}

/** One minute clock for the shell: the time, the sun over Copenhagen and the theme it calls for. */
export function DaylightProvider({
  initialTime,
  children,
}: {
  initialTime: string
  children: React.ReactNode
}): React.JSX.Element {
  const now = useMinute(initialTime)
  const sky = useMemo(() => daylight(now), [now])

  const mode = useSyncExternalStore(
    subscribeToMode,
    readStoredMode,
    () => "sun" as const
  )

  const applied = useRef<boolean | undefined>(undefined)

  useEffect(() => {
    const root = document.documentElement
    // Hydration renders the server's "sun" first; the stored choice already painted wins.
    const stored = readStoredMode()

    const night =
      stored === "night" || (stored === "sun" && sky.phase === "night")

    // The room turns slowly at dusk and dawn; a first render only syncs.
    if (applied.current !== undefined && applied.current !== night) {
      root.classList.add("theme-turning")
      setTimeout(() => root.classList.remove("theme-turning"), 1400)
    }

    root.classList.toggle("dark", night)
    applied.current = night
  }, [mode, sky.phase])

  const state = useMemo<DaylightState>(
    () => ({ now, sky, mode, setMode: storeMode }),
    [now, sky, mode]
  )

  return <DaylightContext value={state}>{children}</DaylightContext>
}

/**
 * The palette for canvases that cannot read CSS tokens. It reads the applied class after
 * hydration, so a stored choice made before this render is respected.
 */
export function useMapTheme(): "day" | "night" {
  const { mode, sky } = useDaylight()

  const hydrated = useSyncExternalStore(
    subscribeToNothing,
    () => true,
    () => false
  )

  if (!hydrated) return "night"

  return mode === "night" || (mode === "sun" && sky.phase === "night")
    ? "night"
    : "day"
}

const modeChange = "openhome-theme-change"

function subscribeToMode(onChange: () => void): () => void {
  window.addEventListener(modeChange, onChange)
  window.addEventListener("storage", onChange)

  return () => {
    window.removeEventListener(modeChange, onChange)
    window.removeEventListener("storage", onChange)
  }
}

function subscribeToNothing(): () => void {
  return () => {}
}

function readStoredMode(): ThemeMode {
  try {
    const stored = localStorage.getItem(storageKey)

    return stored === "day" || stored === "night" ? stored : "sun"
  } catch {
    return "sun"
  }
}

function storeMode(next: ThemeMode): void {
  try {
    if (next === "sun") localStorage.removeItem(storageKey)
    else localStorage.setItem(storageKey, next)
  } catch {
    // Private browsing: the choice cannot be kept, so the theme keeps following the sun.
  }

  window.dispatchEvent(new Event(modeChange))
}

const sunPosition = (progress: number): number =>
  Math.min(1, Math.max(0, progress))

/** The shell's baseline, with the sun at its place between sunrise and sunset. */
export function Horizon(): React.JSX.Element {
  const { sky } = useDaylight()
  const below = sky.progress < 0 || sky.progress > 1

  return (
    <div
      className="horizon"
      aria-hidden="true"
      // SAFETY: CSSProperties omits custom properties; --sun-x is a unitless 0..1 horizon position read by .horizon.
      style={
        {
          "--sun-x": below
            ? sky.progress > 1
              ? 1
              : 0
            : sunPosition(sky.progress),
        } as React.CSSProperties
      }
    >
      {below ? null : <span className="horizon-trail" />}
      <span className="horizon-sun" data-below={below} />
    </div>
  )
}

const modes: Record<
  ThemeMode,
  { next: ThemeMode; label: string; Icon: typeof SunIcon }
> = {
  sun: { next: "day", label: "Theme follows the sun", Icon: SunHorizonIcon },
  day: { next: "night", label: "Day theme", Icon: SunIcon },
  night: { next: "sun", label: "Night theme", Icon: MoonIcon },
}

/** Cycles follow-the-sun, day and night. */
export function ThemeToggle(): React.JSX.Element {
  const { mode, setMode } = useDaylight()
  const { next, label, Icon } = modes[mode]

  return (
    <button
      type="button"
      className="icon-link"
      aria-label={`${label}. Switch to ${modes[next].label.toLowerCase()}.`}
      title={label}
      onClick={() => setMode(next)}
    >
      <Icon
        size={20}
        weight={mode === "sun" ? "regular" : "fill"}
        aria-hidden="true"
      />
    </button>
  )
}

const sunTime = new Intl.DateTimeFormat("da-DK", {
  hour: "2-digit",
  minute: "2-digit",
  hour12: false,
  timeZone: "Europe/Copenhagen",
})

/**
 * Today's arc: sunrise to sunset as a curve, the sun on it now. At night the sun waits below
 * the line as a hollow ring at the side it will rise from or has set on.
 */
export function SunDay(): React.JSX.Element {
  const { sky } = useDaylight()
  const below = sky.progress < 0 || sky.progress > 1
  const x = below ? (sky.progress > 1 ? 1 : 0) : sky.progress
  // A half-sine arc reads as a sky path; the dot follows the same curve.
  const y = below ? 1.18 : 1 - Math.sin(x * Math.PI) * 0.86

  const points = Array.from({ length: 49 }, (_, index) => {
    const at = index / 48

    return `${(at * 100).toFixed(2)},${((1 - Math.sin(at * Math.PI) * 0.86) * 100).toFixed(2)}`
  })

  const lit = points.slice(0, Math.round(sunPosition(x) * 48) + 1)

  return (
    <div className="sun-day">
      <span>
        <span className="sr-only">Sunrise </span>
        <span className="num">
          {sky.sunrise === null ? "–" : sunTime.format(sky.sunrise)}
        </span>
      </span>
      <div className="sun-day-track" aria-hidden="true">
        <svg viewBox="0 0 100 100" preserveAspectRatio="none">
          <line x1="0" x2="100" y1="100" y2="100" />
          <path d={`M${points.join(" L")}`} />
          {below || lit.length < 2 ? null : (
            <path className="sun-day-lit" d={`M${lit.join(" L")}`} />
          )}
        </svg>
        <span
          className="sun-day-dot"
          data-below={below}
          // SAFETY: CSSProperties omits custom properties; --x and --y are unitless 0..1 track coordinates read by .sun-day-dot.
          style={{ "--x": x, "--y": y } as React.CSSProperties}
        />
      </div>
      <span>
        <span className="sr-only">Sunset </span>
        <span className="num">
          {sky.sunset === null ? "–" : sunTime.format(sky.sunset)}
        </span>
      </span>
    </div>
  )
}
