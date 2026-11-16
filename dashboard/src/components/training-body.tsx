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

/** The body signals and journal for the selected day. */
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
  // Days up to the selected one, oldest first: the baseline never looks ahead of the day shown.
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
      <section className="today-vitals" aria-label="Body">
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
        <ul className="today-journal" aria-label={`Journal · ${data.day}`}>
          {data.journal.map((entry) => (
            <li key={entry.question} data-yes={entry.answered_yes}>
              <span aria-hidden="true">{entry.answered_yes ? "✓" : "✕"}</span>
              <span>
                {entry.question}
                <span className="sr-only">
                  : {entry.answered_yes ? "Yes" : "No"}
                </span>
              </span>
              {entry.numeric_value === null ? null : (
                <strong>{formatDecimal(entry.numeric_value)}</strong>
              )}
              {entry.notes === null ? null : <em>{entry.notes}</em>}
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

/** One body signal: a glyph, today's figure, and how far it sits from its own recent days. */
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
    <div className="vital" data-tone={tone}>
      <VitalIcon className="vital-icon" size={22} aria-hidden="true" />
      <p className="sr-only">{label}</p>
      <p className="vital-value num">
        <span data-gap={value === null}>{figure}</span>
        {value === null || unit === "" ? null : <small>{unit}</small>}
      </p>
      {delta === null ? null : (
        <p className="vital-delta num">
          <span aria-hidden="true">{delta >= 0 ? "▲" : "▼"}</span>
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

  // Within 3 % of the recent average reads as steady rather than a change.
  if (Math.abs(delta) <= Math.abs(baseline) * 0.03) return "fair"

  return delta > 0 === (better === "higher") ? "good" : "poor"
}
