import { Link } from "@tanstack/react-router"
import { CalendarBlankIcon } from "@phosphor-icons/react/dist/csr/CalendarBlank"
import { ChartLineIcon } from "@phosphor-icons/react/dist/csr/ChartLine"

/**
 * Links in the Food page's switches are current only for their exact search, so Router's own
 * active state never marks a second choice beside the chosen one.
 */
export const exactSearch = {
  exact: true,
  includeSearch: true,
  explicitUndefined: true,
} as const

const views = [
  { view: "day", label: "Day", Icon: CalendarBlankIcon },
  { view: "trends", label: "Trends", Icon: ChartLineIcon },
] as const

/** The Food page's Day and Trends switch; Day keeps a clean URL and opens on today. */
export function FoodViews({
  view,
}: {
  readonly view: (typeof views)[number]["view"]
}): React.JSX.Element {
  return (
    <nav className="view-tabs food-views" aria-label="Food views">
      {views.map((option) => (
        <Link
          key={option.view}
          to="/food"
          search={option.view === "day" ? {} : { view: "trends" }}
          activeOptions={exactSearch}
          className="view-tab"
          aria-current={option.view === view ? "page" : undefined}
        >
          <option.Icon size={20} weight="bold" aria-hidden="true" />
          {option.label}
        </Link>
      ))}
    </nav>
  )
}
