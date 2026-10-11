import { createFileRoute, Link, notFound } from "@tanstack/react-router"
import { ArrowLeftIcon } from "@phosphor-icons/react/dist/csr/ArrowLeft"
import { Card, CardContent, CardHeader } from "@/components/ui/card"
import { features, type Feature } from "@/lib/features"

export const Route = createFileRoute("/$feature")({
  loader: ({ params }) => getFeature(params.feature).id,
  head: ({ loaderData }) => ({
    meta: [
      {
        title: `${loaderData === undefined ? "Page unavailable" : getFeature(loaderData).title} · OpenHome`,
      },
    ],
  }),
  component: FeaturePage,
})

function FeaturePage(): React.JSX.Element {
  const feature = getFeature(Route.useLoaderData())

  return (
    <section className="focused-page">
      <header>
        <p className="page-eyebrow">OpenHome / {feature.label}</p>
        <h1 id="page-heading" tabIndex={-1}>
          {feature.title}
        </h1>
        <p className="feature-intro">{feature.description}</p>
      </header>
      <Card>
        <CardHeader>
          <feature.icon
            size={40}
            aria-hidden="true"
            className="text-muted-foreground"
          />
          <h2>Not available yet</h2>
        </CardHeader>
        <CardContent>
          <p>{feature.unavailable}</p>
          <p className="feature-help">
            Checking the home connection will not enable this feature. It will
            become available when its integration is added.
          </p>
          <Link to="/" className="action-link">
            <ArrowLeftIcon size={20} aria-hidden="true" />
            Back to Home
          </Link>
        </CardContent>
      </Card>
    </section>
  )
}

function getFeature(id: string): Feature {
  const feature = features.find((item) => item.id === id)

  if (feature === undefined) throw notFound()

  return feature
}
