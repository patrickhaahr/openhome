import {
  idleMilestone,
  milestoneFigure,
  type ImportantDate,
} from "@/lib/important-dates"
import { formatDay } from "@/lib/lists"

/** A passive lead slot; its parent owns loading and midnight refresh. No qualifying date means no markup. */
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
    <div className="idle-milestone">
      <p className="idle-milestone-figure num">{milestoneFigure(date)}</p>
      <p className="idle-milestone-title">
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
