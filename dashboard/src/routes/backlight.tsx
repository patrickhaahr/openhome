import { useEffect } from "react"
import { createFileRoute, getRouteApi, useRouter } from "@tanstack/react-router"
import { BacklightControls } from "@/components/backlight-controls"
import { PageHead } from "@/components/page-head"
import { getBacklight, updateBacklight } from "@/lib/backlight.functions"
import { useMinute } from "@/lib/use-minute"

const rootRoute = getRouteApi("__root__")

export const Route = createFileRoute("/backlight")({
  head: () => ({ meta: [{ title: "LED · OpenHome" }] }),
  loader: () => getBacklight(),
  headers: () => ({ "Cache-Control": "no-store" }),
  component: BacklightPage,
})

function BacklightPage(): React.JSX.Element {
  const data = Route.useLoaderData()
  const { openedAt } = rootRoute.useLoaderData()
  const now = useMinute(openedAt)
  const router = useRouter()

  useEffect(() => {
    const timer = setInterval(() => {
      if (document.visibilityState === "visible")
        void router.invalidate({ sync: true })
    }, 15000)

    return () => clearInterval(timer)
  }, [router])

  return (
    <>
      <PageHead title="LED" />
      <BacklightControls
        data={data}
        now={now}
        change={async (action) => {
          const result = await updateBacklight({ data: action })
          await router.invalidate({ sync: true })

          return result
        }}
      />
    </>
  )
}
