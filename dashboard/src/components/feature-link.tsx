import { Link } from "@tanstack/react-router"
import type { Feature } from "@/lib/features"

export function FeatureLink({
  feature,
  className,
  title,
  children = feature.label,
  onClick,
}: {
  feature: Feature
  className?: string
  title?: string
  children?: React.ReactNode
  onClick?: () => void
}): React.JSX.Element {
  const to = implementedRoute(feature.id)

  return to === undefined ? (
    <Link
      to="/$feature"
      params={{ feature: feature.id }}
      className={className}
      title={title}
      onClick={onClick}
      data-soon=""
    >
      {children}
    </Link>
  ) : (
    <Link to={to} className={className} title={title} onClick={onClick}>
      {children}
    </Link>
  )
}

export function implementedRoute(
  id: Feature["id"]
):
  | "/controls"
  | "/weather"
  | "/travel"
  | "/lists"
  | "/binder"
  | "/photos"
  | "/workout"
  | "/recovery"
  | "/food"
  | undefined {
  switch (id) {
    case "controls":
      return "/controls"
    case "weather":
      return "/weather"
    case "travel":
      return "/travel"
    case "lists":
      return "/lists"
    case "binder":
      return "/binder"
    case "photos":
      return "/photos"
    case "workout":
      return "/workout"
    case "recovery":
      return "/recovery"
    case "food":
      return "/food"
    case "agent":
    case "server":
      return undefined
  }
}
