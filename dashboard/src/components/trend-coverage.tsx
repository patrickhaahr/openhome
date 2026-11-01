import {
  coverageRows,
  cutDownPeriods,
  type Aggregation,
  type CoverageRow,
  type MicronutrientCoverage,
  type SummaryPeriod,
} from "@/lib/nutrition-trends"
import { formatPercent } from "@/lib/nutrition"

const groupNames: Record<CoverageRow["group"], string> = {
  vitamins: "Vitamins",
  minerals: "Minerals",
  fatty_acids: "Omega-3",
}

const periodWords: Record<Aggregation, readonly [string, string]> = {
  day: ["day", "days"],
  week: ["week", "weeks"],
  month: ["month", "months"],
}

function count(value: number, [one, many]: readonly [string, string]): string {
  return `${value} ${value === 1 ? one : many}`
}

/** Periods wider than this get no gap between their cells, so a year of days still fits. */
const gappedPeriods = 60

type CoverageTrendsProps = {
  readonly coverage: MicronutrientCoverage
  readonly periods: readonly SummaryPeriod[]
  readonly by: Aggregation
  /** Full name per period, for the table and the cut down note. */
  readonly names: readonly string[]
  /** Short axis label per period. */
  readonly labels: readonly string[]
}

/**
 * The Trends view's vitamins and minerals: each nutrient's mean share of its Recommended Intake
 * a logged day as a strip of period columns (ink once reached, red in a cut down period, an amber
 * dot under a period with a day above an Upper Limit) and its range mean, with Axum's lasting
 * shortfalls and cut downs named in the summary and marked on their rows. The strips are hidden
 * from assistive technology, which reads the summary and a table of every nutrient instead.
 */
export function CoverageTrends({
  coverage,
  periods,
  by,
  names,
  labels,
}: CoverageTrendsProps): React.JSX.Element {
  if (coverage.state === "unavailable")
    return (
      <section
        className="trend-coverage"
        data-state="unavailable"
        aria-labelledby="trend-coverage-heading"
      >
        <h3 id="trend-coverage-heading">No vitamin and mineral coverage yet</h3>
        <p>{coverage.reason}.</p>
        {coverage.missing.length === 0 ? null : (
          <p className="trend-summary">
            Sex and birth date are set on the Workout page&apos;s Body view.
          </p>
        )}
      </section>
    )

  const rows = coverageRows(coverage.nutrients, periods)
  const short = rows.filter((row) => row.lasting_shortfall)

  const cutDown = rows.filter(
    (row) => row.seven_day_average_above_upper_limit === true
  )

  const { below_percent: below, min_logged_days: minDays } = coverage.shortfall

  const summary = [
    short.length === 0
      ? "No lasting shortfall"
      : `Lasting shortfall: ${short.map((row) => row.name).join(", ")}`,
    cutDown.length === 0
      ? "nothing to cut down"
      : `Cut down: ${cutDown.map((row) => row.name).join(", ")}`,
  ].join(" · ")

  const gap = periods.length > gappedPeriods ? "0px" : "2px"

  return (
    <section
      className="trend-coverage"
      aria-labelledby="trend-coverage-heading"
    >
      <header>
        <h3 id="trend-coverage-heading">Vitamins and minerals</h3>
        <p className="trend-summary">{summary}</p>
        <p className="trend-coverage-basis">
          Mean share of the Recommended Intake a logged day over{" "}
          {count(coverage.logged_days, ["logged day", "logged days"])} · a
          lasting shortfall is below {below} % over at least {minDays} logged
          days · cut down when a 7-day average passes an Upper Limit
        </p>
        <ul className="trend-legend trend-coverage-legend" aria-hidden="true">
          <li data-tone="ink">Reached</li>
          <li data-tone="muted">Below</li>
          <li data-tone="danger">Cut down</li>
          <li data-tone="caution" data-kind="dots">
            A day above an Upper Limit
          </li>
        </ul>
      </header>
      {(["vitamins", "minerals", "fatty_acids"] as const).map((group) => {
        const members = rows.filter((row) => row.group === group)

        return members.length === 0 ? null : (
          <section key={group} className="trend-coverage-group">
            <h4>{groupNames[group]}</h4>
            <ul className="trend-coverage-rows">
              {members.map((row) => (
                <CoverageRowItem
                  key={row.key}
                  row={row}
                  by={by}
                  names={names}
                  partial={periods.map((period) => period.partial)}
                  gap={gap}
                />
              ))}
            </ul>
          </section>
        )
      })}
      <p className="trend-coverage-axis" aria-hidden="true">
        <span>{labels.at(0)}</span>
        <span>{labels.at(-1)}</span>
      </p>
      <table className="sr-only">
        <caption>Vitamins and minerals over the range</caption>
        <thead>
          <tr>
            <th scope="col">Nutrient</th>
            <th scope="col">Mean of the Recommended Intake</th>
            <th scope="col">Lasting shortfall</th>
            <th scope="col">Days above an Upper Limit</th>
            <th scope="col">Cut down in</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => {
            const periodsCut = cutDownPeriods(row, names)

            return (
              <tr key={row.key}>
                <th scope="row">
                  {row.name}
                  {row.adequate_intake ? ", adequate intake" : ""}
                </th>
                <td>
                  {row.percent_of_recommended === null
                    ? "Unknown"
                    : `${formatPercent(row.percent_of_recommended)} over ${count(row.averaged_days, ["logged day", "logged days"])}`}
                </td>
                <td>{row.lasting_shortfall ? "Yes" : "No"}</td>
                <td>
                  {row.days_above_upper_limit === null
                    ? "No Upper Limit"
                    : row.days_above_upper_limit}
                </td>
                <td>
                  {row.seven_day_average_above_upper_limit === null
                    ? "No Upper Limit"
                    : periodsCut.length === 0
                      ? "None"
                      : periodsCut.join(", ")}
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>
      <p className="trend-coverage-basis">
        {coverage.sources.map((source) => source.citation).join(" · ")}
      </p>
    </section>
  )
}

/** The note under a row: its cut down periods in red, or its days above an Upper Limit. */
function limitNote(row: CoverageRow, by: Aggregation): string | null {
  const above = row.days_above_upper_limit ?? 0
  const cut = row.cells.filter((cell) => cell.limit === "cut_down").length

  const days =
    above === 0
      ? null
      : `${count(above, ["day", "days"])} above the Upper Limit`

  if (cut > 0)
    return [`Cut down in ${count(cut, periodWords[by])}`, days]
      .filter((part): part is string => part !== null)
      .join(" · ")

  return days
}

function CoverageRowItem({
  row,
  by,
  names,
  partial,
  gap,
}: {
  readonly row: CoverageRow
  readonly by: Aggregation
  readonly names: readonly string[]
  readonly partial: readonly boolean[]
  readonly gap: string
}): React.JSX.Element {
  const note = limitNote(row, by)
  const cut = row.seven_day_average_above_upper_limit === true

  return (
    <li
      data-key={row.key}
      data-shortfall={row.lasting_shortfall ? "" : undefined}
      data-limit={note === null ? undefined : cut ? "cut_down" : "above"}
    >
      <span className="trend-coverage-name">
        {row.name}
        {row.adequate_intake ? (
          <span
            className="food-micro-ai"
            title="Adequate intake: a less certain reference than a recommended intake"
          >
            <span aria-hidden="true">AI</span>
          </span>
        ) : null}
      </span>
      <span
        className="trend-coverage-strip"
        aria-hidden="true"
        // SAFETY: CSSProperties omits custom properties; --periods and --gap lay out the columns.
        style={
          { "--periods": row.cells.length, "--gap": gap } as React.CSSProperties
        }
      >
        {row.cells.map((cell, index) => (
          <span
            key={names[index] ?? index}
            data-gap={cell.percent === null ? "" : undefined}
            data-met={
              cell.percent !== null && cell.percent >= 100 ? "" : undefined
            }
            data-limit={cell.limit === "clear" ? undefined : cell.limit}
            data-partial={partial[index] === true ? "" : undefined}
            // SAFETY: CSSProperties omits custom properties; --fill is the reached share, capped at 1.
            style={
              {
                "--fill": Math.min((cell.percent ?? 0) / 100, 1),
              } as React.CSSProperties
            }
          />
        ))}
      </span>
      <span className="trend-coverage-percent num">
        {row.percent_of_recommended === null ? (
          <span data-gap="true" />
        ) : (
          formatPercent(row.percent_of_recommended)
        )}
        {row.lasting_shortfall ? (
          <>
            {" "}
            <span className="trend-coverage-short">Short</span>
          </>
        ) : null}
      </span>
      {note === null ? null : (
        <span className="trend-coverage-limit" aria-hidden="true">
          {note}
        </span>
      )}
    </li>
  )
}
