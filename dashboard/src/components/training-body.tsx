import { FootprintsIcon } from "@phosphor-icons/react/dist/csr/Footprints"
import { HeartIcon } from "@phosphor-icons/react/dist/csr/Heart"
import { ThermometerIcon } from "@phosphor-icons/react/dist/csr/Thermometer"
import { WaveSineIcon } from "@phosphor-icons/react/dist/csr/WaveSine"
import { WindIcon } from "@phosphor-icons/react/dist/csr/Wind"
import type { Icon } from "@phosphor-icons/react/dist/lib/types"
import { formatDecimal, formatSigned, formatWhole } from "@/lib/number-format"
import { type Band } from "@/lib/recovery-score"
import {
  type MetricTrend,
  type RecoveryDay,
  type TrainingRead,
  type TrainingState,
} from "@/lib/training"

export function Body({
  data,
  week,
  trends,
  missing,
}: {
  readonly data: RecoveryDay
  readonly week: TrainingState["week"]
  readonly trends: TrainingState["trends"]
  readonly missing: string
}): React.JSX.Element {
  const weekly = (pick: (day: RecoveryDay) => number | null) =>
    week
      .filter((entry) => entry.day <= data.day)
      .map((entry) => ({
        day: entry.day,
        value:
          entry.recovery.state === "available"
            ? pick(entry.recovery.data)
            : null,
      }))

  return (
    <>
      <section
        className="grid grid-cols-5 gap-3 max-[1100px]:grid-cols-3 max-[700px]:grid-cols-2"
        aria-label="Body"
      >
        <Vital
          icon={WaveSineIcon}
          label="HRV"
          unit="ms"
          value={data.hrv_rmssd.value}
          points={trendPoints(trends.hrv)}
          better="higher"
          missing={missing}
        />
        <Vital
          icon={HeartIcon}
          label="Resting HR"
          unit="bpm"
          value={data.resting_hr.value}
          points={trendPoints(trends.resting_heart_rate)}
          better="lower"
          missing={missing}
        />
        <Vital
          icon={WindIcon}
          label="Breathing"
          unit="/min"
          value={data.respiratory_rate.value}
          points={weekly((day) => day.respiratory_rate.value)}
          better="neither"
          missing={missing}
        />
        <Vital
          icon={ThermometerIcon}
          label="Skin temp"
          unit="°C"
          value={data.skin_temp_deviation.value}
          points={weekly((day) => day.skin_temp_deviation.value)}
          better="neither"
          signed
          missing={missing}
        />
        <Vital
          icon={FootprintsIcon}
          label="Steps"
          unit=""
          value={data.steps.value}
          points={weekly((day) => day.steps.value)}
          better="neither"
          grouped
          missing={missing}
        />
      </section>
      {data.journal.length === 0 ? null : (
        <ul
          className="flex flex-wrap gap-2"
          aria-label={`Journal · ${data.day}`}
        >
          {data.journal.map((entry) => (
            <li
              key={entry.question}
              className="inline-flex min-h-10 items-center gap-2 rounded-full bg-tile py-1.5 pr-4 pl-3 text-control"
            >
              <span
                className={`grid size-6 shrink-0 place-items-center rounded-full text-caption font-bold ${entry.answered_yes ? "bg-ink text-ground" : "bg-wash-strong"}`}
                aria-hidden="true"
              >
                {entry.answered_yes ? "✓" : "✕"}
              </span>
              <span>
                {entry.question}
                <span className="sr-only">
                  : {entry.answered_yes ? "Yes" : "No"}
                </span>
              </span>
              {entry.numeric_value === null ? null : (
                <strong>{formatDecimal(entry.numeric_value)}</strong>
              )}
              {entry.notes === null ? null : (
                <em className="text-muted-ink not-italic">{entry.notes}</em>
              )}
            </li>
          ))}
        </ul>
      )}
    </>
  )
}

function trendPoints(
  read: TrainingRead<MetricTrend>
): ReadonlyArray<{ readonly day: string; readonly value: number | null }> {
  return read.state === "available" ? read.data.daily_points : []
}

function Vital({
  icon: VitalIcon,
  label,
  unit,
  value,
  points,
  better,
  signed = false,
  grouped = false,
  missing,
}: {
  readonly icon: Icon
  readonly label: string
  readonly unit: string
  readonly value: number | null
  readonly points: ReadonlyArray<{
    readonly day: string
    readonly value: number | null
  }>
  readonly better: "higher" | "lower" | "neither"
  readonly signed?: boolean
  readonly grouped?: boolean
  readonly missing: string
}): React.JSX.Element {
  const earlier = points
    .slice(0, -1)
    .flatMap((point) => (point.value === null ? [] : [point.value]))

  const baseline =
    earlier.length < 3
      ? null
      : earlier.reduce((sum, item) => sum + item, 0) / earlier.length

  const delta = value === null || baseline === null ? null : value - baseline
  const tone = toneOf(delta, better, baseline)
  const format = grouped ? formatWhole : formatDecimal

  const figure =
    value === null ? missing : signed ? formatSigned(value) : format(value)

  return (
    <div
      data-slot="vital"
      data-band={tone}
      className="grid min-h-30 grid-cols-trail items-center gap-y-3 rounded-sheet bg-tile px-5 py-4.5 max-[700px]:last:odd:col-span-full"
    >
      <VitalIcon
        className="col-start-1 row-start-1 text-muted-ink"
        size={22}
        aria-hidden="true"
      />
      <p className="sr-only">{label}</p>
      <p className="col-span-full row-start-2 self-end text-vital leading-none font-semibold tracking-figure tabular-nums">
        <span
          data-gap={value === null}
          className="data-[gap=true]:gap-dash-5.5"
        >
          {figure}
        </span>
        {value === null || unit === "" ? null : (
          <small className="ml-0.75 text-small font-medium tracking-normal text-muted-ink">
            {unit}
          </small>
        )}
      </p>
      {delta === null ? null : (
        <p
          className={`col-start-2 row-start-1 inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-label font-bold tracking-figure tabular-nums ${tone === "good" || tone === "poor" ? "bg-band/14 text-band" : "bg-wash-strong text-muted-ink"}`}
        >
          <span className="text-mark" aria-hidden="true">
            {delta >= 0 ? "▲" : "▼"}
          </span>
          <span className="sr-only">
            {delta >= 0 ? "Above" : "Below"} its recent average by{" "}
          </span>
          {signed
            ? formatSigned(Math.abs(delta)).replace("+", "")
            : format(Math.abs(delta))}
        </p>
      )}
    </div>
  )
}

function toneOf(
  delta: number | null,
  better: "higher" | "lower" | "neither",
  baseline: number | null
): Band {
  if (delta === null || baseline === null || better === "neither") return "none"

  if (Math.abs(delta) <= Math.abs(baseline) * 0.03) return "fair"

  return delta > 0 === (better === "higher") ? "good" : "poor"
}
