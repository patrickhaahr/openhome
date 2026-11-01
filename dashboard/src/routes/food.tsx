import { useEffect, useRef } from "react"
import { createFileRoute, useRouter } from "@tanstack/react-router"
import { FoodDay } from "@/components/food-day"
import { FoodTrends } from "@/components/food-trends"
import { PageHead } from "@/components/page-head"
import { nutritionSearch } from "@/lib/nutrition"
import {
  loadNutritionDay,
  loadNutritionSummary,
  saveNutritionGoal,
} from "@/lib/nutrition.functions"
import { trendsChoiceOf, trendsSearch } from "@/lib/nutrition-trends"
import { useCopenhagenDay } from "@/lib/use-copenhagen-day"

/**
 * Food: one Copenhagen day's targets, Meal Logs and totals, opening on today, or the Trends of
 * the last 30 to 365 days.
 */
export const Route = createFileRoute("/food")({
  head: () => ({ meta: [{ title: "Food · OpenHome" }] }),
  validateSearch: nutritionSearch.extend(trendsSearch.shape),
  loaderDeps: ({ search }) => ({
    view: search.view,
    day: search.day,
    range: search.range,
    by: search.by,
  }),
  loader: async ({ deps }) =>
    deps.view === "trends"
      ? {
          view: "trends" as const,
          data: await loadNutritionSummary(trendsChoiceOf(deps)),
        }
      : {
          view: "day" as const,
          data: await loadNutritionDay({ day: deps.day }),
        },
  staleTime: 0,
  headers: () => ({ "Cache-Control": "no-store" }),
  component: FoodPage,
})

function FoodPage(): React.JSX.Element {
  const loaded = Route.useLoaderData()
  const search = Route.useSearch()
  const router = useRouter()
  const today = useCopenhagenDay()
  const shown = useRef(today)

  // Without a chosen day the page follows today, so it re-reads at Copenhagen midnight; Trends
  // always end today.
  useEffect(() => {
    if (shown.current === today) return
    shown.current = today

    if (search.day === undefined) void router.invalidate({ sync: true })
  }, [today, search.day, router])

  return (
    <>
      <PageHead title="Food" />
      {loaded.view === "trends" ? (
        <FoodTrends data={loaded.data} />
      ) : (
        <FoodDay
          data={loaded.data}
          saveGoal={(goal) => saveNutritionGoal({ data: goal })}
          reload={() => router.invalidate({ sync: true })}
        />
      )}
    </>
  )
}
