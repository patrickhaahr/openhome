import {
  coverageRows,
  cutDownPeriods,
  type Aggregation,
  type CoverageRow,
  type MicronutrientCoverage,
  type SummaryPeriod,
} from "@/lib/nutrition-trends"
import { formatPercent } from "@/lib/nutrition"
import { adequateIntake, groupHeading } from "@/components/food-amount"
import { TrendLegend, trendSummary, trendTitle } from "@/components/trend-parts"
import { cn } from "@/lib/utils"

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

const panel = "grid col-span-full min-w-0 gap-4 rounded-xl bg-wash p-food-plate"

const basis = "text-label text-muted-ink"

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
        data-slot="trend-coverage"
        className={panel}
        data-state="unavailable"
        aria-labelledby="trend-coverage-heading"
      >
        <h2 id="trend-coverage-heading" className={trendTitle}>
          No vitamin and mineral coverage yet
        </h2>
        <p>{coverage.reason}.</p>
        {coverage.missing.length === 0 ? null : (
          <p data-slot="trend-summary" className={trendSummary}>
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
      data-slot="trend-coverage"
      className={panel}
      aria-labelledby="trend-coverage-heading"
    >
      <header className="grid gap-1">
        <h2 id="trend-coverage-heading" className={trendTitle}>
          Vitamins and minerals
        </h2>
        <p data-slot="trend-summary" className={trendSummary}>
          {summary}
        </p>
        <p data-slot="trend-coverage-basis" className={basis}>
          Mean share of the Recommended Intake a logged day over{" "}
          {count(coverage.logged_days, ["logged day", "logged days"])} · a
          lasting shortfall is below {below} % over at least {minDays} logged
          days · cut down when a 7-day average passes an Upper Limit
        </p>
        <TrendLegend
          items={[
            { key: "ink", name: "Reached", tone: "ink", kind: "bars" },
            { key: "muted", name: "Below", tone: "muted", kind: "bars" },
            { key: "danger", name: "Cut down", tone: "danger", kind: "bars" },
            {
              key: "caution",
              name: "A day above an Upper Limit",
              tone: "caution",
              kind: "dots",
            },
          ]}
        />
      </header>
      {(["vitamins", "minerals", "fatty_acids"] as const).map((group) => {
        const members = rows.filter((row) => row.group === group)

        return members.length === 0 ? null : (
          <section key={group} data-slot="trend-coverage-group">
            <h3 className={groupHeading}>{groupNames[group]}</h3>
            <ul data-slot="trend-coverage-rows" className="grid gap-2.5">
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
      <p
        className="-mt-1 flex justify-between pr-30 pl-food-axis text-caption text-muted-ink tabular-nums max-[40rem]:p-0"
        aria-hidden="true"
      >
        <span>{labels.at(0)}</span>
        <span>{labels.at(-1)}</span>
      </p>
      <div className="sr-only">
        <table>
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
      </div>
      <p data-slot="trend-coverage-basis" className={basis}>
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

/** A period's column: its share of the Recommended Intake, red when cut down, none when not logged. */
function cellClass(
  cell: CoverageRow["cells"][number],
  partial: boolean
): string {
  const fill =
    cell.limit === "cut_down"
      ? "min-h-1.5 bg-danger"
      : cell.percent === null
        ? "min-h-0"
        : cell.percent >= 100
          ? "min-h-0.5 bg-ink"
          : "min-h-0.5 bg-muted-ink"

  return cn(
    "relative h-share rounded-t-xs",
    fill,
    cell.limit === "above" &&
      "after:absolute after:-bottom-2 after:left-1/2 after:size-1.25 after:-translate-x-1/2 after:rounded-full after:bg-food-caution",
    partial && "opacity-45"
  )
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
      className="grid grid-cols-coverage items-end gap-x-4 gap-y-0.5 max-[40rem]:grid-cols-trail"
    >
      <span className="flex items-baseline gap-2 leading-compact font-semibold">
        {row.name}
        {row.adequate_intake ? (
          <span
            className={adequateIntake}
            title="Adequate intake: a less certain reference than a recommended intake"
          >
            <span aria-hidden="true">AI</span>
          </span>
        ) : null}
      </span>
      <span
        data-slot="trend-coverage-strip"
        className="grid h-7 grid-cols-periods items-end gap-x-(--gap) rounded-food-swatch bg-wash-strong max-[40rem]:col-span-full max-[40rem]:row-start-2"
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
            data-limit={cell.limit === "clear" ? undefined : cell.limit}
            className={cellClass(cell, partial[index] === true)}
            // SAFETY: CSSProperties omits custom properties; --share is the reached share, capped at 1.
            style={
              {
                "--share": Math.min((cell.percent ?? 0) / 100, 1),
              } as React.CSSProperties
            }
          />
        ))}
      </span>
      <span
        data-slot="trend-coverage-percent"
        className="flex items-baseline justify-end gap-1.5 text-large leading-compact font-medium num"
      >
        {row.percent_of_recommended === null ? (
          <span data-gap="true" className="data-[gap=true]:gap-dash" />
        ) : (
          formatPercent(row.percent_of_recommended)
        )}
        {row.lasting_shortfall ? (
          <>
            {" "}
            <span className="rounded-full bg-food-caution px-2 py-px text-caption font-semibold tracking-normal text-food-caution-ink">
              Short
            </span>
          </>
        ) : null}
      </span>
      {note === null ? null : (
        <span
          data-slot="trend-coverage-limit"
          className={cn(
            "col-start-2 -col-end-1 mt-2 text-label font-semibold max-[40rem]:col-span-full",
            cut ? "text-danger" : "text-ink"
          )}
          aria-hidden="true"
        >
          {note}
        </span>
      )}
    </li>
  )
}
