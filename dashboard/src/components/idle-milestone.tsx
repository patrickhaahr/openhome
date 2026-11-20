import {
  idleMilestone,
  milestoneFigure,
  type ImportantDate,
} from "@/lib/important-dates"
import { formatDay } from "@/lib/lists"

export function IdleMilestone({
  dates,
}: {
  dates: readonly ImportantDate[]
}): React.JSX.Element | null {
  const date = idleMilestone(dates)

  if (date === undefined) return null

  const yearlyYears =
    date.milestone?.kind === "yearly" ? date.milestone.years : 0

  return (
    <div data-slot="idle-milestone" className="grid max-w-120 min-w-0 gap-1.5">
      <p className="text-idle-milestone leading-poster font-medium num text-balance wrap-anywhere idle-short:text-idle-milestone-short">
        {milestoneFigure(date)}
      </p>
      <p className="text-idle-milestone-title leading-compact wrap-anywhere">
        {date.milestone?.kind === "yearly" ? (
          <span className="num">
            {yearlyYears > 0
              ? `${yearlyYears} ${yearlyYears === 1 ? "year" : "years"} since `
              : ""}
            <time dateTime={date.date}>{formatDay(date.date)}</time>
          </span>
        ) : (
          date.title
        )}
      </p>
    </div>
  )
}
