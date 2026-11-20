import { cva } from "class-variance-authority"
import type { Icon } from "@phosphor-icons/react"
import { cn } from "@/lib/utils"

export function ListsFrame({
  className,
  ...props
}: React.ComponentProps<"div">): React.JSX.Element {
  return <div className={cn("grid gap-flow", className)} {...props} />
}

export function ListBar(props: React.ComponentProps<"div">): React.JSX.Element {
  return (
    <div
      data-slot="lists-bar"
      className="flex min-w-0 flex-wrap items-center gap-x-4 gap-y-3"
      {...props}
    />
  )
}

export function ListBarEnd({
  className,
  ...props
}: React.ComponentProps<"div">): React.JSX.Element {
  return (
    <div
      className={cn("ml-auto flex items-center gap-2", className)}
      {...props}
    />
  )
}

export function ListEmpty({
  icon: Glyph,
  title,
  children,
}: {
  icon: Icon
  title: React.ReactNode
  children: React.ReactNode
}): React.JSX.Element {
  return (
    <div
      data-slot="lists-empty"
      className="grid justify-items-start gap-3 rounded-sheet bg-tile p-callout text-muted-ink [&_svg]:text-ink"
    >
      <Glyph size={48} weight="light" aria-hidden="true" />
      <h2 className="text-lead font-medium tracking-title text-ink">{title}</h2>
      {children}
    </div>
  )
}

export const keyLink =
  "inline-flex min-h-11 items-center rounded-full bg-wash-strong px-5 font-medium text-ink no-underline"

export function BoardHeading({
  count,
  noun,
  hidden = false,
  children,
}: {
  count: number
  noun: readonly [string, string]
  hidden?: boolean
  children: React.ReactNode
}): React.JSX.Element {
  return (
    <h2
      className={
        hidden
          ? "sr-only"
          : "flex items-center gap-2.5 text-lead leading-none font-medium tracking-title"
      }
    >
      {children}
      <span className="text-base leading-none font-medium num text-muted-ink">
        {count}
        <span className="sr-only"> {count === 1 ? noun[0] : noun[1]}</span>
      </span>
    </h2>
  )
}

export const listKeys = "flex flex-wrap gap-2"

export const listKey = cva(
  "inline-flex min-h-11 items-center gap-2 rounded-full px-4.5 text-control font-medium text-muted-ink transition-colors duration-fast ease-app hover:bg-wash-strong hover:text-ink disabled:opacity-40 aria-pressed:bg-wash-strong aria-pressed:font-semibold aria-pressed:text-ink",
  {
    variants: { kind: { list: "bg-wash", new: "bg-transparent" } },
    defaultVariants: { kind: "list" },
  }
)

export function WishMark(): React.JSX.Element {
  return (
    <i
      className="absolute top-3 right-3 size-3.5 rounded-full bg-sun"
      aria-hidden="true"
    />
  )
}
