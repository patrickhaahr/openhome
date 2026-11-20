import { cn } from "@/lib/utils"
import type { ControlLayout } from "@/lib/command-feedback"

const shell = {
  quick:
    "flex flex-col items-center gap-2 rounded-full bg-wash p-2 max-phone:flex-row **:data-[slot=button]:size-14.5 **:data-[slot=button]:[&_svg:not([class*=size-])]:size-7",
  full: "relative flex min-w-0 flex-col items-center gap-5 bg-wash **:data-[icon-only=true]:[&_svg:not([class*=size-])]:size-6 **:data-[slot=button]:[&.bg-secondary]:enabled:not-hover:not-data-[command=ok]:bg-ground **:data-[slot=button]:[&.bg-secondary]:enabled:not-hover:not-data-[command=ok]:bg-clip-border",
} satisfies Record<ControlLayout, string>

export const alertOnPlate = "max-w-16 pt-2 pb-1 text-center whitespace-normal"

export function ControlPanel({
  layout,
  className,
  labelledBy,
  children,
}: {
  layout: ControlLayout
  className?: string | undefined
  labelledBy: string
  children: React.ReactNode
}): React.JSX.Element {
  return (
    <div
      className={cn(shell[layout], className)}
      role="group"
      aria-labelledby={labelledBy}
    >
      {children}
    </div>
  )
}

export function PanelHead({
  layout,
  bare = false,
  children,
}: {
  layout: ControlLayout
  bare?: boolean
  children: React.ReactNode
}): React.JSX.Element {
  return (
    <div
      className={
        layout === "quick" || bare
          ? "contents"
          : "flex flex-col items-center gap-2"
      }
    >
      {children}
    </div>
  )
}
