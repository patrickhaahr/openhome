import * as React from "react"
import { cn } from "cn"
import { CaretDownIcon } from "@phosphor-icons/react/dist/csr/CaretDown"

type NativeSelectProps = Omit<React.ComponentProps<"select">, "size"> & {
  size?: "sm" | "default"
}

function NativeSelect({
  className,
  size = "default",
  ...props
}: NativeSelectProps) {
  return (
    <div
      className={cn(
        "group/native-select relative w-fit has-[select:disabled]:opacity-50",
        className
      )}
      data-slot="native-select-wrapper"
      data-size={size}
    >
      <select
        data-slot="native-select"
        data-size={size}
        className="h-11 w-full min-w-0 cursor-pointer appearance-none rounded-full border border-input bg-transparent py-2 pr-10 pl-4 text-[0.9375rem] font-medium transition-[color,border-color,background-color] outline-none select-none selection:bg-primary selection:text-primary-foreground placeholder:text-muted-foreground hover:border-primary focus-visible:border-ring disabled:pointer-events-none disabled:cursor-not-allowed aria-invalid:border-destructive data-[size=sm]:h-9"
        {...props}
      />
      <CaretDownIcon
        className="pointer-events-none absolute top-1/2 right-4 size-3.5 -translate-y-1/2 text-muted-foreground select-none"
        aria-hidden="true"
        data-slot="native-select-icon"
      />
    </div>
  )
}

function NativeSelectOption({
  className,
  ...props
}: React.ComponentProps<"option">) {
  return (
    <option
      data-slot="native-select-option"
      className={cn("bg-[Canvas] text-[CanvasText]", className)}
      {...props}
    />
  )
}

function NativeSelectOptGroup({
  className,
  ...props
}: React.ComponentProps<"optgroup">) {
  return (
    <optgroup
      data-slot="native-select-optgroup"
      className={cn("bg-[Canvas] text-[CanvasText]", className)}
      {...props}
    />
  )
}

export { NativeSelect, NativeSelectOptGroup, NativeSelectOption }
