import * as React from "react"
import { Input as InputPrimitive } from "@base-ui/react/input"
import { cva, type VariantProps } from "class-variance-authority"
import { cn } from "cn"

const inputVariants = cva(
  "h-11 w-full min-w-0 rounded-[0.75rem] border border-transparent bg-secondary px-3.5 py-1 text-base transition-[color,border-color,background-color] outline-none file:inline-flex file:h-7 file:border-0 file:bg-transparent file:text-sm file:font-medium file:text-foreground placeholder:text-muted-foreground hover:border-input focus-visible:border-ring focus-visible:bg-transparent disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 aria-invalid:border-destructive",
  {
    variants: {
      variant: {
        default: "",
        /** One large figure typed in place, such as a body weight. */
        figure:
          "h-14 w-[6.5ch] rounded-[1rem] px-3 text-right text-[2rem] font-semibold tracking-[-0.04em] text-foreground tabular-nums placeholder:text-line-strong",
      },
    },
    defaultVariants: { variant: "default" },
  }
)

function Input({
  className,
  type,
  variant,
  ...props
}: React.ComponentProps<"input"> & VariantProps<typeof inputVariants>) {
  return (
    <InputPrimitive
      type={type}
      data-slot="input"
      className={cn(inputVariants({ variant }), className)}
      {...props}
    />
  )
}

export { Input }
