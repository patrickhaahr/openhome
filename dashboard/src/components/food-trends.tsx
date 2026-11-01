import { Link } from "@tanstack/react-router"
import { exactSearch, FoodViews } from "@/components/food-views"
import { TrendChart } from "@/components/trend-chart"
import { CoverageTrends } from "@/components/trend-coverage"
import { formatDecimal, formatSigned, formatWhole } from "@/lib/number-format"
import { foodDatabaseCredit } from "@/lib/nutrition"
import {
  aggregations,
  axisLabel,
  defaultAggregation,
  formatKcal,
  formatWeekly,
  novaShareOf,
  periodName,
  trendRanges,
  trendsFailureMessage,
  weightedMean,
  type Aggregation,
  type NutritionSummary,
  type NutritionSummaryState,
  type SummaryPeriod,
  type TrendRange,
  type TrendsChoice,
} from "@/lib/nutrition-trends"

const aggregationNames: Record<Aggregation, string> = {
  day: "Days",
  week: "Weeks",
  month: "Months",
}

/**
 * The Food page's Trends view: the range and aggregation choices, then each series as its own
 * chart (calories against the Calorie Target and Estimated Expenditure, protein against its
 * target, energy by macronutrient, the NOVA 1 share, Trend Weight against scale weight, and the
 * weekly change against the Nutrition Goal), then the vitamins and minerals against their
 * Recommended Intakes and Upper Limits, with the Food database's credit.
 */
export function FoodTrends({
  data,
}: {
  readonly data: NutritionSummaryState
}): React.JSX.Element {
  return (
    <div className="food">
      <FoodViews view="trends" />
      <TrendControls choice={data} />
      {data.state === "available" ? (
        <Trends summary={data.data} />
      ) : (
        <div className="state-note food-unavailable" role="status">
          <h2>Trends unavailable</h2>
          <p>{trendsFailureMessage[data.reason]}</p>
        </div>
      )}
      <p className="food-credit">
        Nutrient values {foodDatabaseCredit} (CC BY 4.0)
      </p>
    </div>
  )
}

/** The Food page's Trends search; a choice's defaults stay out of the URL. */
type TrendsLink = {
  view: "trends"
  range?: TrendRange
  by?: Aggregation
}

/** Search parameters for a choice; defaults stay out of the URL. */
function searchFor(choice: TrendsChoice): TrendsLink {
  const search: TrendsLink = { view: "trends" }

  if (choice.range !== 30) search.range = choice.range

  if (choice.by !== defaultAggregation(choice.range)) search.by = choice.by

  return search
}

function TrendControls({
  choice,
}: {
  readonly choice: TrendsChoice
}): React.JSX.Element {
  return (
    <div className="food-trend-controls">
      <nav className="view-tabs" aria-label="Range">
        {trendRanges.map((range) => (
          <Link
            key={range}
            to="/food"
            search={searchFor({ range, by: defaultAggregation(range) })}
            activeOptions={exactSearch}
            className="view-tab"
            aria-current={range === choice.range ? "page" : undefined}
          >
            {range}
            <span className="food-trend-unit"> days</span>
          </Link>
        ))}
      </nav>
      <nav className="view-tabs" aria-label="Group by">
        {aggregations.map((by) => (
          <Link
            key={by}
            to="/food"
            search={searchFor({ range: choice.range, by })}
            activeOptions={exactSearch}
            className="view-tab"
            aria-current={by === choice.by ? "page" : undefined}
          >
            {aggregationNames[by]}
          </Link>
        ))}
      </nav>
    </div>
  )
}

function percent(share: number): string {
  return `${Math.round(share * 100)} %`
}

function grams(value: number): string {
  return `${formatWhole(value)} g`
}

function kilograms(value: number): string {
  return `${formatDecimal(value)} kg`
}

/** The last known value of a series, with the first, for a change over the range. */
function ends(
  periods: readonly SummaryPeriod[],
  value: (period: SummaryPeriod) => number | null
): { first: number; last: number } | null {
  const known = periods.flatMap((period) => {
    const entry = value(period)

    return entry === null ? [] : [entry]
  })

  const first = known.at(0)
  const last = known.at(-1)

  return first === undefined || last === undefined ? null : { first, last }
}

function Trends({
  summary,
}: {
  readonly summary: NutritionSummary
}): React.JSX.Element {
  const { periods, aggregation: by } = summary
  const days = summary.logged_days + summary.missing_days

  const common = {
    labels: periods.map((period) => axisLabel(period, by)),
    names: periods.map((period) => periodName(period, by)),
    partial: periods.map((period) => period.partial),
  }

  // Intake means cover the logged days whose energy is known.
  const intakeDays = (period: SummaryPeriod) => period.intake_days
  const all = (period: SummaryPeriod) => period.days
  const notLogged = "Not logged"

  const eaten = weightedMean(periods, (p) => p.energy.eaten_kcal, intakeDays)
  const target = weightedMean(periods, (p) => p.energy.calorie_target_kcal, all)

  const expenditure = weightedMean(
    periods,
    (p) => p.energy.estimated_expenditure_kcal,
    all
  )

  const caloriesSummary = [
    eaten === null
      ? "No Meal Logs in this range"
      : `${formatKcal(eaten)} a logged day`,
    target === null
      ? "no Calorie Target yet"
      : `Calorie Target ${formatKcal(target)}`,
    expenditure === null
      ? null
      : `Estimated Expenditure ${formatKcal(expenditure)}`,
  ]
    .filter((part): part is string => part !== null)
    .join(" · ")

  const protein = weightedMean(periods, (p) => p.protein.eaten_g, intakeDays)
  const proteinTarget = weightedMean(periods, (p) => p.protein.target_g, all)

  const proteinSummary = [
    protein === null
      ? "No Meal Logs in this range"
      : `${grams(protein)} a logged day`,
    proteinTarget === null ? null : `target ${grams(proteinTarget)}`,
  ]
    .filter((part): part is string => part !== null)
    .join(" · ")

  const macros = [
    {
      key: "protein",
      name: "Protein",
      tone: "sun",
      pick: (p: SummaryPeriod) => p.macro_energy.protein_kcal,
    },
    {
      key: "carbs",
      name: "Carbs",
      tone: "ink",
      pick: (p: SummaryPeriod) => p.macro_energy.carbohydrate_kcal,
    },
    {
      key: "fat",
      name: "Fat",
      tone: "fat",
      pick: (p: SummaryPeriod) => p.macro_energy.fat_kcal,
    },
    {
      key: "alcohol",
      name: "Alcohol",
      tone: "muted",
      pick: (p: SummaryPeriod) => p.macro_energy.alcohol_kcal,
    },
  ] as const

  const macroMeans = macros.map((macro) => ({
    name: macro.name,
    kcal: weightedMean(periods, macro.pick, intakeDays) ?? 0,
  }))

  const macroTotal = macroMeans.reduce((sum, macro) => sum + macro.kcal, 0)

  const macroSummary =
    macroTotal > 0
      ? macroMeans
          .flatMap((macro) =>
            macro.kcal > 0
              ? [`${macro.name} ${percent(macro.kcal / macroTotal)}`]
              : []
          )
          .join(" · ")
      : "No Meal Logs in this range"

  const nova = novaShareOf(periods)
  const weight = ends(periods, (p) => p.weight.trend_weight_kg)
  const change = ends(periods, (p) => p.weight_change.kg_per_week)
  const goal = periods.at(-1)?.weight_change.goal_kg_per_week

  return (
    <div className="food-trend-charts">
      <p className="food-trend-coverage">
        <span className="num">{summary.logged_days}</span> of{" "}
        <span className="num">{days}</span> days logged
        <span className="sr-only">
          ; days without Meal Logs are left out of the means, never counted as
          zero
        </span>
      </p>
      <TrendChart
        {...common}
        chart="calories"
        title="Calories"
        summary={caloriesSummary}
        zero
        format={formatKcal}
        tick={formatWhole}
        marks={[
          {
            kind: "bars",
            key: "eaten",
            name: "Eaten",
            tone: "ink",
            gap: notLogged,
            values: periods.map((p) => p.energy.eaten_kcal),
          },
          {
            kind: "line",
            key: "target",
            name: "Calorie Target",
            tone: "sun",
            values: periods.map((p) => p.energy.calorie_target_kcal),
          },
          {
            kind: "line",
            key: "expenditure",
            name: "Estimated Expenditure",
            tone: "muted",
            dashed: true,
            values: periods.map((p) => p.energy.estimated_expenditure_kcal),
          },
        ]}
      />
      <TrendChart
        {...common}
        chart="protein"
        title="Protein"
        summary={proteinSummary}
        zero
        format={grams}
        tick={formatWhole}
        marks={[
          {
            kind: "bars",
            key: "eaten",
            name: "Eaten",
            tone: "sun",
            gap: notLogged,
            values: periods.map((p) => p.protein.eaten_g),
          },
          {
            kind: "line",
            key: "target",
            name: "Target",
            tone: "ink",
            values: periods.map((p) => p.protein.target_g),
          },
        ]}
      />
      <TrendChart
        {...common}
        chart="macros"
        title="Energy by macronutrient"
        summary={macroSummary}
        zero
        format={formatKcal}
        tick={formatWhole}
        marks={[
          {
            kind: "stack",
            parts: macros.map((macro) => ({
              key: macro.key,
              name: macro.name,
              tone: macro.tone,
              gap: notLogged,
              values: periods.map(macro.pick),
            })),
          },
        ]}
      />
      <TrendChart
        {...common}
        chart="nova"
        title="NOVA 1 share"
        summary={
          nova === null
            ? "Nothing classified yet"
            : `${percent(nova)} of classified energy unprocessed or minimally processed`
        }
        domain={[0, 100]}
        format={(value) => `${Math.round(value)} %`}
        marks={[
          {
            kind: "line",
            key: "share",
            name: "NOVA 1",
            tone: "ink",
            values: periods.map((p) =>
              p.nova.nova_1_share === null ? null : p.nova.nova_1_share * 100
            ),
          },
        ]}
      />
      <TrendChart
        {...common}
        chart="weight"
        title="Weight"
        summary={
          weight === null
            ? "No Body Weight yet"
            : `Trend Weight ${kilograms(weight.last)} · ${formatSigned(weight.last - weight.first)} kg over the range`
        }
        format={kilograms}
        tick={formatDecimal}
        marks={[
          {
            kind: "dots",
            key: "scale",
            name: "Scale weight",
            tone: "faint",
            values: periods.map((p) => p.weight.scale_weight_kg),
          },
          {
            kind: "line",
            key: "trend",
            name: "Trend Weight",
            tone: "sun",
            values: periods.map((p) => p.weight.trend_weight_kg),
          },
        ]}
      />
      <TrendChart
        {...common}
        chart="weekly-change"
        title="Weekly change"
        summary={[
          change === null
            ? "No weekly change yet"
            : `Latest ${formatWeekly(change.last)}`,
          goal === undefined ? null : `goal ${formatWeekly(goal)}`,
        ]
          .filter((part): part is string => part !== null)
          .join(" · ")}
        zero
        format={formatWeekly}
        tick={(value) => formatDecimal(value)}
        marks={[
          {
            kind: "bars",
            key: "change",
            name: "Trend Weight change",
            tone: "ink",
            values: periods.map((p) => p.weight_change.kg_per_week),
          },
          {
            kind: "line",
            key: "goal",
            name: "Goal",
            tone: "sun",
            values: periods.map((p) => p.weight_change.goal_kg_per_week),
          },
        ]}
      />
      <CoverageTrends
        coverage={summary.micronutrients}
        periods={periods}
        by={by}
        names={common.names}
        labels={common.labels}
      />
    </div>
  )
}
