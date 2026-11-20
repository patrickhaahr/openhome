import { cva, type VariantProps } from "class-variance-authority"
import { cn } from "@/lib/utils"

const track = cva("flex max-w-full gap-1 rounded-full bg-wash p-1", {
  variants: {
    variant: {
      tabs: "w-max overflow-x-auto max-sm:w-full",
      bar: "w-max overflow-visible max-sm:w-full",
      people: "relative w-max overflow-x-auto max-sm:order-1 max-sm:w-full",
      segments: "w-full overflow-x-auto",
      fit: "w-max overflow-x-auto",
    },
  },
  defaultVariants: { variant: "tabs" },
})

const tab = cva(
  "group/tab inline-flex min-h-11 items-center gap-2 rounded-full px-4.5 text-control font-medium whitespace-nowrap transition-colors duration-fast ease-app hover:bg-wash-strong hover:text-ink aria-pressed:bg-ink aria-pressed:text-ground aria-[current=page]:bg-ink aria-[current=page]:text-ground aria-[current=page]:[&_svg]:text-sun",
  {
    variants: {
      variant: {
        tab: "text-muted-ink max-sm:flex-1 max-sm:flex-col max-sm:justify-center max-sm:gap-0.5",
        segment:
          "flex-1 justify-center text-muted-ink max-sm:flex-col max-sm:gap-0.5",
        person: "flex-none text-muted-ink max-sm:pr-4 max-sm:pl-1.5",
        chip: "flex-none bg-wash-strong text-ink max-sm:pr-4 max-sm:pl-1.5",
      },
    },
    defaultVariants: { variant: "tab" },
  }
)

type ViewTabsProps = React.ComponentProps<"div"> &
  VariantProps<typeof track> & {
    as?: "div" | "nav"
  }

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

export function viewTab(variant?: VariantProps<typeof tab>["variant"]) {
  return { "data-slot": "view-tab", className: tab({ variant }) } as const
}
