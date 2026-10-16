import { createFileRoute, notFound } from "@tanstack/react-router"
import { PageHead } from "@/components/page-head"
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
    <section className="soon">
      <PageHead title={feature.title} />
      <feature.icon className="soon-glyph" weight="thin" aria-hidden="true" />
      <h2 className="soon-title">Not available yet</h2>
      <div className="sr-only">
        <p>{feature.unavailable}</p>
        <p>
          Checking the home connection will not enable this feature. It will
          become available when its integration is added.
        </p>
      </div>
    </section>
  )
}

function getFeature(id: string): Feature {
  const feature = features.find((item) => item.id === id)

  if (feature === undefined) throw notFound()

  return feature
}
