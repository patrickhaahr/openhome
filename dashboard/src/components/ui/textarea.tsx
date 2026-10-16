import * as React from "react"
import { cn } from "cn"

function Textarea({ className, ...props }: React.ComponentProps<"textarea">) {
  return (
    <textarea
      data-slot="textarea"
      className={cn(
        "flex field-sizing-content min-h-20 w-full resize-none rounded-[0.75rem] border border-transparent bg-secondary px-3.5 py-3 text-base transition-[color,border-color,background-color] outline-none placeholder:text-muted-foreground hover:border-input focus-visible:border-ring focus-visible:bg-transparent disabled:cursor-not-allowed disabled:opacity-50 aria-invalid:border-destructive",
        className
      )}
      {...props}
    />
  )
}

export { Textarea }
