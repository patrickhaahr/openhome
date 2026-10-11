import { Link } from "@tanstack/react-router"
import { ArrowUpRightIcon } from "@phosphor-icons/react/dist/csr/ArrowUpRight"
import { Card, CardContent, CardHeader } from "@/components/ui/card"
import type { Feature } from "@/lib/features"

type FeatureCardProps = { readonly feature: Feature }

export function FeatureCard({ feature }: FeatureCardProps): React.JSX.Element {
  return (
    <Link
      to="/$feature"
      params={{ feature: feature.id }}
      className="feature-link"
      aria-label={`Open ${feature.title}`}
    >
      <Card>
        <CardHeader>
          <div className="feature-card-top">
            <feature.icon size={28} aria-hidden="true" />
            <ArrowUpRightIcon size={20} aria-hidden="true" />
          </div>
          <h2 className="feature-card-title">{feature.title}</h2>
        </CardHeader>
        <CardContent>
          <p className="feature-card-description">{feature.description}</p>
          <p className="availability-label">
            <span aria-hidden="true" />
            Not available yet
          </p>
        </CardContent>
      </Card>
    </Link>
  )
}
