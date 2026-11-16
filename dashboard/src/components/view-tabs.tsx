import { cva, type VariantProps } from "class-variance-authority"
import { cn } from "@/lib/utils"

/** The wash capsule that holds a row of tabs. */
const track = cva("flex max-w-full gap-1 rounded-full bg-wash p-1", {
  variants: {
    variant: {
      /** A page's view switch: equal tabs across a phone. */
      tabs: "w-max overflow-x-auto max-sm:w-full",
      /** The Workout toolbar's switch; its focus rings stay outside the track. */
      bar: "w-max overflow-visible max-sm:w-full",
      /** One key per Household User; a single scrolling row on a phone. */
      people: "relative w-max overflow-x-auto max-sm:order-1 max-sm:w-full",
      /** A choice inside a sheet, filling the sheet's width. */
      segments: "w-full overflow-x-auto",
      /** A choice that is only as wide as its segments. */
      fit: "w-max overflow-x-auto",
    },
  },
  defaultVariants: { variant: "tabs" },
})

/** Ink pill when chosen (`aria-current` for a link, `aria-pressed` for a button). */
const tab = cva(
  "group/tab inline-flex min-h-11 items-center gap-2 rounded-full px-4.5 text-control font-medium whitespace-nowrap transition-colors duration-fast ease-app hover:bg-wash-strong hover:text-ink aria-pressed:bg-ink aria-pressed:text-ground aria-[current=page]:bg-ink aria-[current=page]:text-ground aria-[current=page]:[&_svg]:text-sun",
  {
    variants: {
      variant: {
        /** Icon over word on a phone. */
        tab: "text-muted-ink max-sm:flex-1 max-sm:flex-col max-sm:justify-center max-sm:gap-0.5",
        segment:
          "flex-1 justify-center text-muted-ink max-sm:flex-col max-sm:gap-0.5",
        /** A person's disc and name; never stacks. */
        person: "flex-none text-muted-ink max-sm:pr-4 max-sm:pl-1.5",
        /** A person key on its own, without a track: a tone step even when unchosen. */
        chip: "flex-none bg-wash-strong text-ink max-sm:pr-4 max-sm:pl-1.5",
      },
    },
    defaultVariants: { variant: "tab" },
  }
)

type ViewTabsProps = React.ComponentProps<"div"> &
  VariantProps<typeof track> & {
    /** `nav` for a link switch, which needs an `aria-label`. */
    as?: "div" | "nav"
  }

/** A segmented control: pick a `variant`, put `ViewTabs` around items spread from `viewTab`. */
export function ViewTabs({
  as: Tag = "div",
  variant,
  className,
  ...props
}: ViewTabsProps): React.JSX.Element {
  return (
    <Tag
      data-slot="view-tabs"
      className={cn(track({ variant }), className)}
      {...props}
    />
  )
}

/** Props for one item (a `Link` or a `button`) inside `ViewTabs`. */
export function viewTab(variant?: VariantProps<typeof tab>["variant"]) {
  return { "data-slot": "view-tab", className: tab({ variant }) } as const
}
