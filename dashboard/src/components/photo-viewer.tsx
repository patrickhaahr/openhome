import { CaretLeftIcon } from "@phosphor-icons/react/dist/csr/CaretLeft"
import { CaretRightIcon } from "@phosphor-icons/react/dist/csr/CaretRight"
import { XIcon } from "@phosphor-icons/react/dist/csr/X"
import { Button } from "@/components/ui/button"
import { DialogClose } from "@/components/ui/dialog"
import { cn } from "@/lib/utils"

export function PhotoViewer({
  className,
  ...props
}: React.ComponentProps<"div">): React.JSX.Element {
  return <div className={cn("absolute inset-0", className)} {...props} />
}

export function ViewerStage({
  className,
  ...props
}: React.ComponentProps<"div">): React.JSX.Element {
  return (
    <div
      className={cn("absolute inset-0 touch-pinch-zoom", className)}
      {...props}
    />
  )
}

export function ViewerBar({
  decorative = false,
  children,
}: {
  decorative?: boolean
  children: React.ReactNode
}): React.JSX.Element {
  return (
    <div className="pointer-events-none absolute inset-x-0 top-0 flex items-center justify-between gap-4 p-tools-room *:pointer-events-auto">
      <span
        className="flex min-w-0 flex-wrap items-baseline gap-x-3.5 gap-y-1 rounded-full bg-photo-plate/80 px-4 py-2.5 text-control text-photo-ink/82"
        aria-hidden={decorative ? true : undefined}
      >
        {children}
      </span>
      <DialogClose
        render={<Button variant="viewer" size="key-icon" />}
        title="Close photo"
      >
        <XIcon size={22} aria-hidden="true" />
        <span className="sr-only">Close photo</span>
      </DialogClose>
    </div>
  )
}

export function ViewerStep({
  side,
  disabled,
  onClick,
}: {
  side: "previous" | "next"
  disabled: boolean
  onClick: () => void
}): React.JSX.Element {
  const previous = side === "previous"

  return (
    <div
      className={cn(
        "absolute top-1/2 -translate-y-1/2 has-disabled:invisible max-phone:top-auto max-phone:bottom-5 max-phone:translate-y-0",
        previous ? "left-tools-room" : "right-tools-room"
      )}
    >
      <Button
        variant="viewer"
        size="key-icon"
        className="size-14"
        disabled={disabled}
        title={previous ? "Previous photo" : "Next photo"}
        onClick={onClick}
      >
        {previous ? (
          <CaretLeftIcon size={26} aria-hidden="true" />
        ) : (
          <CaretRightIcon size={26} aria-hidden="true" />
        )}
        <span className="sr-only">{previous ? "← Previous" : "Next →"}</span>
      </Button>
    </div>
  )
}
