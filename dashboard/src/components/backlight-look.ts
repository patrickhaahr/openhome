import type { BacklightStatus } from "@/lib/backlight"

export type Light = { readonly fill: string; readonly motion: string }

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

export function lightFor(mode: string, effect: string | undefined): Light {
  if (mode === "tv-sync") return tvSync

  return (effect === undefined ? undefined : effects.get(effect)) ?? colour
}

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

export const signal = {
  reachable: "bg-led-ink",
  unknown: "border-2 border-led-muted",
  unconfigured: "border-2 border-led-muted",
  unreachable: "border-2 border-led-danger motion-safe:animate-led-refuse",
} satisfies Record<BacklightStatus["reachability"], string>
