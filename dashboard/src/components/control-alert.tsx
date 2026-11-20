import { Link } from "@tanstack/react-router"
import { PlugsIcon } from "@phosphor-icons/react/dist/csr/Plugs"
import { WarningCircleIcon } from "@phosphor-icons/react/dist/csr/WarningCircle"
import { cn } from "@/lib/utils"

export function ControlAlert({
  tag,
  className,
}: {
  tag: string
  className?: string | undefined
}): React.JSX.Element {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 text-small font-semibold tracking-nav whitespace-nowrap text-danger",
        className
      )}
      aria-hidden="true"
    >
      <WarningCircleIcon size={18} weight="fill" />
      {tag}
    </span>
  )
}

export function OpenConnection(): React.JSX.Element {
  return (
    <Link
      to="/connection"
      className="inline-grid size-12 place-items-center rounded-full bg-wash-strong text-danger transition-colors duration-fast ease-app hover:bg-ink hover:text-ground"
      title="Open Connection"
    >
      <PlugsIcon size={20} aria-hidden="true" />
      <span className="sr-only">Open Connection</span>
    </Link>
  )
}
