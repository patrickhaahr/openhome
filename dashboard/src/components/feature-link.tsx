import { Link } from "@tanstack/react-router"
import type { Feature } from "@/lib/features"

/** A link to one feature; its face is the label unless the caller draws one. */
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

  // Implemented features use their own typed routes; the rest share the placeholder.
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
  | "/photos"
  | "/workout"
  | "/recovery"
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
    case "photos":
      return "/photos"
    case "workout":
      return "/workout"
    case "recovery":
      return "/recovery"
    case "agent":
    case "server":
      return undefined
  }
}
