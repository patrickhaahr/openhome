import { Button as ButtonPrimitive } from "@base-ui/react/button"
import { cva, type VariantProps } from "class-variance-authority"
import { cn } from "cn"

const answers = [
  "data-[outcome=pending]:disabled:opacity-100 data-outcome:after:pointer-events-none data-outcome:after:absolute data-outcome:after:-inset-1.25 data-outcome:after:rounded-[inherit]",
  "data-[outcome=pending]:after:border-2 data-[outcome=pending]:after:border-transparent data-[outcome=pending]:after:border-t-sun data-[outcome=pending]:after:border-r-sun",
  "data-[outcome=accepted]:after:border-2 data-[outcome=accepted]:after:border-sun",
  "data-[outcome=failed]:outline-2 data-[outcome=failed]:outline-offset-3 data-[outcome=failed]:outline-solid data-[outcome=failed]:outline-danger",
  "motion-safe:data-[outcome=pending]:after:animate-key-spin motion-safe:data-[outcome=accepted]:animate-key-acknowledge motion-safe:data-[outcome=accepted]:after:animate-key-ripple motion-safe:data-[outcome=accepted]:after:opacity-0 motion-safe:data-[outcome=failed]:animate-led-refuse",
  "data-[spinning=true]:[&_svg]:animate-key-spin",
]

const buttonVariants = cva(
  [
    answers,
    "group/button relative inline-flex shrink-0 items-center justify-center rounded-full border border-transparent bg-clip-padding text-[0.9375rem] font-medium tracking-[-0.01em] whitespace-nowrap transition-[background-color,color,border-color,scale] duration-200 ease-[cubic-bezier(.2,.7,.1,1)] outline-none select-none focus-visible:outline-2 focus-visible:outline-offset-3 focus-visible:outline-ring active:not-aria-[haspopup]:scale-[0.97] disabled:pointer-events-none disabled:opacity-40 aria-invalid:border-destructive [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-[1.125rem]",
  ],
  {
    variants: {
      variant: {
        default:
          "bg-primary text-primary-foreground hover:bg-[color-mix(in_srgb,var(--primary),var(--sun)_22%)]",
        outline:
          "bg-secondary text-secondary-foreground hover:bg-primary hover:text-primary-foreground aria-expanded:bg-primary aria-expanded:text-primary-foreground aria-pressed:bg-primary aria-pressed:text-primary-foreground",
        secondary:
          "bg-secondary text-secondary-foreground hover:bg-primary hover:text-primary-foreground aria-expanded:bg-primary aria-expanded:text-primary-foreground",
        ghost:
          "text-muted-foreground hover:bg-secondary hover:text-foreground aria-expanded:bg-secondary aria-expanded:text-foreground",
        destructive:
          "bg-destructive/12 bg-clip-border text-destructive hover:bg-destructive hover:text-background",
        link: "rounded-none px-0 text-primary underline decoration-1 underline-offset-[0.3em] hover:decoration-2",
        loop: "grid w-full grid-cols-[0.75rem_auto_minmax(0,1fr)_auto] items-center justify-start rounded-full border-0 bg-transparent text-left text-base font-normal whitespace-normal text-ink hover:bg-wash aria-pressed:bg-wash-strong",
        ink: "bg-primary text-primary-foreground",
        viewer:
          "bg-photo-plate/80 bg-clip-border text-photo-ink hover:bg-photo-ink hover:text-photo-night",
        tone: "border-0 bg-wash bg-clip-border text-ink hover:bg-wash-strong focus-visible:outline-offset-2",
      },
      size: {
        default:
          "h-11 gap-2 px-5 has-data-[icon=inline-end]:pr-4 has-data-[icon=inline-start]:pl-4",
        xs: "h-7 gap-1 px-3 text-xs has-data-[icon=inline-end]:pr-2 has-data-[icon=inline-start]:pl-2 [&_svg:not([class*='size-'])]:size-3",
        sm: "h-9 gap-1.5 px-4 text-sm has-data-[icon=inline-end]:pr-3 has-data-[icon=inline-start]:pl-3",
        lg: "h-13 gap-2 px-6 has-data-[icon=inline-end]:pr-5 has-data-[icon=inline-start]:pl-5",
        icon: "size-11",
        "icon-xs": "size-7 [&_svg:not([class*='size-'])]:size-3",
        "icon-sm": "size-9",
        "icon-lg": "size-13",
        key: "h-12 min-w-12 gap-2 px-[1.125rem]",
        "key-icon": "size-12",
        "key-xl": "h-18 gap-2.5 px-8 text-lg",
        "key-round": "size-16 text-lg",
        "key-hero": "size-24",
        plate: "h-14 min-w-12 gap-2 px-[1.125rem] max-phone:h-13",
        "plate-lg": "h-14 min-w-12 gap-2 px-7 text-[1.0625rem] max-phone:h-13",
        "map-key": "size-14 max-phone:size-11",
        loop: "min-h-14 gap-3 px-4 py-2 max-phone:min-h-12",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  }
)

function Button({
  className,
  variant = "default",
  size = "default",
  ...props
}: ButtonPrimitive.Props & VariantProps<typeof buttonVariants>) {
  return (
    <ButtonPrimitive
      data-slot="button"
      className={cn(buttonVariants({ variant, size, className }))}
      {...props}
    />
  )
}

export { Button, buttonVariants }
