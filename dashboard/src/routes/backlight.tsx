import { useEffect } from "react"
import { createFileRoute, Link, useRouter } from "@tanstack/react-router"
import { BacklightControls } from "@/components/backlight-controls"
import { PageHead } from "@/components/page-head"
import { getBacklight, updateBacklight } from "@/lib/backlight.functions"

export const Route = createFileRoute("/backlight")({
  head: () => ({ meta: [{ title: "LED · OpenHome" }] }),
  loader: () => getBacklight(),
  headers: () => ({ "Cache-Control": "no-store" }),
  component: BacklightPage,
})

function BacklightPage(): React.JSX.Element {
  const data = Route.useLoaderData()
  const router = useRouter()

  useEffect(() => {
    const timer = setInterval(() => {
      if (document.visibilityState === "visible")
        void router.invalidate({ sync: true })
    }, 15000)

    return () => clearInterval(timer)
  }, [router])

  return (
    <section className="backlight-page">
      <PageHead title="LED">
        <Link to="/controls">Back to Controls</Link>
      </PageHead>
      <BacklightControls
        data={data}
        change={async (action) => {
          const result = await updateBacklight({ data: action })
          await router.invalidate({ sync: true })

          return result
        }}
      />
    </section>
  )
}
