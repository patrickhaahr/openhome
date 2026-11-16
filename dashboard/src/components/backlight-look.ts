import type { BacklightStatus } from "@/lib/backlight"

/** How a selection paints the wall: the light's background and how it moves. */
export type Light = { readonly fill: string; readonly motion: string }

// Without a gradient the light is the chosen colour itself (the stage's --led).
const colour: Light = { fill: "bg-(--led)", motion: "" }

const tvSync: Light = { fill: "bg-led-tv-sync", motion: "" }

const effects = new Map<string, Light>([
  [
    "forest",
    {
      fill: "bg-led-forest",
      motion: "motion-safe:animate-led-sway",
    },
  ],
  [
    "aurora",
    {
      fill: "bg-led-aurora",
      motion: "motion-safe:animate-led-sway-fast",
    },
  ],
  [
    "rainbow",
    {
      fill: "bg-led-rainbow",
      motion: "motion-safe:animate-led-turn",
    },
  ],
  [
    "fire",
    {
      fill: "bg-led-fire",
      motion: "motion-safe:animate-led-flicker",
    },
  ],
  [
    "party",
    {
      fill: "bg-led-party",
      motion: "motion-safe:animate-led-turn-steps",
    },
  ],
])

/** The light a mode or effect makes; each effect moves the way its light does. */
export function lightFor(mode: string, effect: string | undefined): Light {
  if (mode === "tv-sync") return tvSync

  return (effect === undefined ? undefined : effects.get(effect)) ?? colour
}

/** A dock key drawn as a small disc of its light; `motion` runs while it is chosen. */
export type Chip = { readonly fill: string; readonly motion: string }

const inset = "inset-2 led-phone:inset-1.5"

const chips = new Map<string, Chip>([
  [
    "color",
    { fill: `${inset} border-3 border-led-wall bg-(--swatch)`, motion: "" },
  ],
  ["off", { fill: "inset-3.5 inset-ring-2 inset-ring-led-muted", motion: "" }],
  [
    "tv-sync",
    {
      fill: `${inset} overflow-hidden bg-led-chip-tv-sync before:absolute before:top-14/25 before:left-1/2 before:aspect-square before:w-11/25 before:-translate-1/2 before:rounded-full before:bg-sun before:half-sun`,
      motion: "",
    },
  ],
  [
    "forest",
    {
      fill: `${inset} bg-led-chip-forest`,
      motion: "motion-safe:animate-led-turn-chip",
    },
  ],
  [
    "aurora",
    {
      fill: `${inset} bg-led-chip-aurora`,
      motion: "motion-safe:animate-led-turn-chip",
    },
  ],
  [
    "rainbow",
    {
      fill: `${inset} bg-led-rainbow`,
      motion: "motion-safe:animate-led-turn-chip",
    },
  ],
  ["fire", { fill: `${inset} bg-led-chip-fire`, motion: "" }],
  [
    "party",
    {
      fill: `${inset} bg-led-party`,
      motion: "motion-safe:animate-led-turn-steps",
    },
  ],
])

const noChip: Chip = { fill: inset, motion: "" }

export function chipFor(id: string): Chip {
  return chips.get(id) ?? noChip
}

/** Reachable: a lit dot. Not yet checked: a ring. Unreachable: a red ring that shakes once. */
export const signal = {
  reachable: "bg-led-ink",
  unknown: "border-2 border-led-muted",
  unconfigured: "border-2 border-led-muted",
  unreachable: "border-2 border-led-danger motion-safe:animate-led-refuse",
} satisfies Record<BacklightStatus["reachability"], string>
