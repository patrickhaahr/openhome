import { useEffect, useRef } from "react"
import { createFileRoute, useRouter } from "@tanstack/react-router"
import { FoodDay } from "@/components/food-day"
import { PageHead } from "@/components/page-head"
import { nutritionSearch } from "@/lib/nutrition"
import { loadNutritionDay, saveNutritionGoal } from "@/lib/nutrition.functions"
import { useCopenhagenDay } from "@/lib/use-copenhagen-day"

/** Food: one Copenhagen day's targets, Meal Logs and totals, opening on today. */
export const Route = createFileRoute("/food")({
  head: () => ({ meta: [{ title: "Food · OpenHome" }] }),
  validateSearch: nutritionSearch,
  loaderDeps: ({ search }) => ({ day: search.day }),
  loader: ({ deps }) => loadNutritionDay(deps),
  staleTime: 0,
  headers: () => ({ "Cache-Control": "no-store" }),
  component: FoodPage,
})

function FoodPage(): React.JSX.Element {
  const data = Route.useLoaderData()
  const search = Route.useSearch()
  const router = useRouter()
  const today = useCopenhagenDay()
  const shown = useRef(today)

  // Without a chosen day the page follows today, so it re-reads at Copenhagen midnight.
  useEffect(() => {
    if (shown.current === today) return
    shown.current = today

    if (search.day === undefined) void router.invalidate({ sync: true })
  }, [today, search.day, router])

  return (
    <>
      <PageHead title="Food" />
      <FoodDay
        data={data}
        saveGoal={(goal) => saveNutritionGoal({ data: goal })}
        reload={() => router.invalidate({ sync: true })}
      />
    </>
  )
}
