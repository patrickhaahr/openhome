import { Toggle as TogglePrimitive } from "@base-ui/react/toggle"
import { cva, type VariantProps } from "class-variance-authority"
import { cn } from "cn"

const toggleVariants = cva(
  "group/toggle inline-flex items-center justify-center gap-1.5 rounded-full text-sm font-medium whitespace-nowrap text-muted-foreground transition-[color,background-color,scale] outline-none hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring active:scale-[0.97] disabled:pointer-events-none disabled:opacity-50 aria-invalid:border-destructive aria-pressed:bg-primary aria-pressed:text-primary-foreground data-[pressed]:bg-primary data-[pressed]:text-primary-foreground [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-3.5",
  {
    variants: {
      variant: {
        default: "bg-transparent",
        outline: "border border-input bg-transparent hover:border-primary",
        /** A segment of a wash capsule; the chosen one is the ink pill. */
        capsule:
          "bg-transparent text-[0.9375rem] text-muted-ink hover:bg-wash-strong hover:text-ink aria-pressed:bg-ink aria-pressed:text-ground data-[pressed]:bg-ink data-[pressed]:text-ground",
        /** A segment of a wash track on a Travel plate; ink words, the chosen one the ink pill.
         * The line height stays text-sm's, which a size of its own would otherwise drop. */
        plate:
          "bg-transparent text-[1rem] leading-[calc(1.25/0.875)] text-ink max-phone:text-[0.9375rem]",
      },
      size: {
        default:
          "h-11 min-w-11 px-5 has-data-[icon=inline-end]:pr-4 has-data-[icon=inline-start]:pl-4",
        sm: "h-9 min-w-9 px-4 has-data-[icon=inline-end]:pr-3 has-data-[icon=inline-start]:pl-3",
        lg: "h-11 min-w-11 px-8 has-data-[icon=inline-end]:pr-5 has-data-[icon=inline-start]:pl-5",
        /** Equal segments across a phone. */
        capsule:
          "h-11 min-w-11 px-[1.125rem] max-[700px]:flex-1 max-[700px]:px-1",
        /** 44px keys on a phone, so the planner wraps to two rows. */
        plate:
          "h-12 min-w-12 px-4 max-phone:h-11 max-phone:min-w-11 max-phone:px-2",
        "plate-icon": "size-12 max-phone:size-11",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  }
)

function Toggle({
  className,
  variant = "default",
  size = "default",
  ...props
}: TogglePrimitive.Props & VariantProps<typeof toggleVariants>) {
  return (
    <TogglePrimitive
      data-slot="toggle"
      className={cn(toggleVariants({ variant, size, className }))}
      {...props}
    />
  )
}

export { Toggle, toggleVariants }
