import type { Feature } from "@/lib/features"

export const iconLink =
  "inline-grid size-11 place-items-center rounded-full text-muted-ink transition-colors duration-fast ease-app hover:bg-wash-strong hover:text-ink aria-[current=page]:bg-wash-strong aria-[current=page]:text-ink aria-pressed:bg-wash-strong aria-pressed:text-ink active:scale-94"

export const barClock =
  "px-2 text-body-lg font-medium tracking-figure tabular-nums group-has-[[data-slot=home]]/shell:hidden"

const logo =
  "group/logo touch-manipulation touch-callout-none font-semibold select-none"

export const wordmark = `${logo} inline-flex min-h-11 items-center gap-wordmark-gap text-wordmark tracking-wordmark`

export const railMark = `${logo} inline-flex min-h-11 min-w-11 flex-none items-center justify-center text-xl leading-normal tracking-headline`

export function LogoDot(): React.JSX.Element {
  return (
    <i
      aria-hidden="true"
      className="ml-dot-gap inline-block size-dot translate-y-dot-rest rounded-full bg-sun transition-transform duration-mid ease-app group-hover/logo:-translate-y-dot-lift"
    />
  )
}

export const topTab =
  "group/tab relative inline-flex min-h-11 items-center px-2 text-control font-medium tracking-nav text-muted-ink transition-colors duration-fast ease-app after:absolute after:inset-x-2 after:bottom-tab-line-offset after:h-tab-line after:origin-left after:scale-x-0 after:bg-current after:transition-transform after:duration-mid after:ease-app hover:text-ink hover:after:scale-x-100 aria-[current=page]:font-semibold aria-[current=page]:text-ink aria-[current=page]:after:scale-x-100 data-soon:not-aria-[current=page]:font-normal group-data-[labels=both]/shell:gap-1.5 group-data-[labels=icons]/shell:min-w-11 group-data-[labels=icons]/shell:justify-center max-tablet:group-data-[labels=both]/shell:gap-1 max-tablet:group-data-[labels=both]/shell:px-1.25 max-tablet:group-data-[labels=both]/shell:after:inset-x-1.25"

export const topTabLabel = "group-data-[labels=icons]/shell:sr-only"

const tab =
  "group/tab flex flex-col items-center justify-center text-caption font-medium tracking-tab text-muted-ink transition-colors duration-fast ease-app hover:text-ink aria-[current=page]:font-semibold aria-[current=page]:text-ink data-[active=true]:font-semibold data-[active=true]:text-ink data-soon:font-normal"

export type Face = "rail" | "bar" | "tile"

const links = {
  rail: `${tab} min-h-12 w-14 gap-0.75 leading-none [@media(min-width:641px)_and_(max-height:680px)]:min-h-11`,
  bar: `${tab} min-h-13 size-full gap-0.75 leading-none`,
  tile: "group/tab flex min-h-18 flex-col items-center justify-center gap-1 text-label font-medium text-muted-ink aria-[current=page]:font-semibold aria-[current=page]:text-ink",
} as const satisfies Record<Face, string>

export const tabLink = (face: Face): string => links[face]

const plateStates =
  "group-hover/tab:bg-wash group-aria-[current=page]/tab:bg-wash-strong group-data-[active=true]/tab:bg-wash-strong group-active/tab:scale-94"

const plates = {
  rail: `h-10 w-12 ${plateStates}`,
  bar: `h-8 w-14 ${plateStates}`,
  tile: "h-9 w-14 group-aria-[current=page]/tab:bg-ground",
} as const satisfies Record<Face, string>

type Glyph = "plate" | "inline"

const glyphs = {
  plate: {
    outline:
      "col-start-1 row-start-1 group-aria-[current=page]/tab:invisible group-data-[active=true]/tab:invisible",
    filled:
      "invisible col-start-1 row-start-1 group-aria-[current=page]/tab:visible group-data-[active=true]/tab:visible",
  },
  inline: {
    outline:
      "hidden flex-none group-not-data-[labels=words]/shell:group-not-aria-[current=page]/tab:block",
    filled:
      "hidden flex-none group-not-data-[labels=words]/shell:group-aria-[current=page]/tab:block",
  },
} as const satisfies Record<Glyph, { outline: string; filled: string }>

export function TabGlyphs({
  Icon,
  size = 22,
  glyph = "plate",
}: {
  Icon: Feature["icon"]
  size?: number
  glyph?: Glyph
}): React.JSX.Element {
  return (
    <>
      <Icon size={size} className={glyphs[glyph].outline} aria-hidden="true" />
      <Icon
        size={size}
        weight="fill"
        className={glyphs[glyph].filled}
        aria-hidden="true"
      />
    </>
  )
}

export function TabFace({
  Icon,
  label,
  face,
}: {
  Icon: Feature["icon"]
  label: string
  face: Face
}): React.JSX.Element {
  return (
    <>
      <span
        className={`grid place-items-center rounded-full transition-plate duration-fast ease-app ${plates[face]}`}
        aria-hidden="true"
      >
        <TabGlyphs Icon={Icon} />
      </span>
      <span className={face === "rail" ? "sr-only" : undefined}>{label}</span>
    </>
  )
}
