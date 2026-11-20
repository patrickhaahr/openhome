import { Link } from "@tanstack/react-router"
import { CalendarBlankIcon } from "@phosphor-icons/react/dist/csr/CalendarBlank"
import { ChartLineIcon } from "@phosphor-icons/react/dist/csr/ChartLine"
import { ViewTabs, viewTab } from "@/components/view-tabs"

export const exactSearch = {
  exact: true,
  includeSearch: true,
  explicitUndefined: true,
} as const

const views = [
  { view: "day", label: "Day", Icon: CalendarBlankIcon },
  { view: "trends", label: "Trends", Icon: ChartLineIcon },
] as const

export function FoodViews({
  view,
}: {
  readonly view: (typeof views)[number]["view"]
}): React.JSX.Element {
  return (
    <ViewTabs as="nav" aria-label="Food views">
      {views.map((option) => (
        <Link
          key={option.view}
          to="/food"
          search={option.view === "day" ? {} : { view: "trends" }}
          activeOptions={exactSearch}
          {...viewTab()}
          aria-current={option.view === view ? "page" : undefined}
        >
          <option.Icon size={20} weight="bold" aria-hidden="true" />
          {option.label}
        </Link>
      ))}
    </ViewTabs>
  )
}
