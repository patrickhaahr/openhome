import { z } from "zod"
import type { ConnectionState } from "./connection"

const nullableNumber = z.number().nullable()

const timestamp = z.iso.datetime({ offset: true })

const coverage = z.enum(["covered", "unknown"])

const freshness = z.enum(["confirmed", "partial", "unconfirmed", "unknown"])

const nightCoverage = z.object({
  daily_metrics: coverage,
  sleep_sessions: coverage,
})

const workoutCoverage = z.object({ workouts: coverage })

const syncFields = {
  imported_device_id: z.string(),
  computed_device_id: z.string(),
  last_push_at: timestamp.nullable(),
  freshness,
}

/** An API-selected observation with its actual unit and source; null stays missing. */
export const measurement = z.object({
  value: nullableNumber,
  unit: z.string(),
  source: z.string().nullable(),
})

/** One measurement, interpreted by Axum. */
export type Measurement = z.infer<typeof measurement>

/** The supported Recovery Day read, including selected sleep sessions and Journal Entries. */
export const recoveryDay = z.object({
  day: z.iso.date(),
  time_zone: z.string(),
  sleep: z.object({
    total: measurement,
    deep: measurement,
    rem: measurement,
    light: measurement,
    efficiency: measurement,
    disturbances: measurement,
    sessions: z
      .object({
        start: timestamp,
        end: timestamp,
        duration_min: z.number(),
        stage_min: z
          .object({
            awake: z.number(),
            light: z.number(),
            deep: z.number(),
            rem: z.number(),
          })
          .nullable(),
        efficiency_fraction: nullableNumber,
        resting_hr_bpm: nullableNumber,
        hrv_rmssd_ms: nullableNumber,
        user_edited: z.boolean(),
        staging_sparse: z.boolean().nullable(),
        source: z.string(),
      })
      .array()
      .max(24),
  }),
  resting_hr: measurement,
  hrv_rmssd: measurement,
  skin_temp_deviation: measurement,
  respiratory_rate: measurement,
  derived_scores: z.object({ recovery: measurement, strain: measurement }),
  journal: z
    .object({
      question: z.string(),
      answered_yes: z.boolean(),
      numeric_value: nullableNumber,
      notes: z.string().nullable(),
      source: z.string(),
    })
    .array()
    .max(64),
  noop: z.object({
    ...syncFields,
    coverage: nightCoverage.extend({ journal: coverage }),
  }),
})

/** Recovery Day as returned by Axum. */
export type RecoveryDay = z.infer<typeof recoveryDay>

const noopSelection = z.object({
  source: z.string().min(1),
  start: timestamp,
  sport: z.string().min(1),
})

const noopSummary = noopSelection.extend({
  end: timestamp,
  origin: z.enum([
    "manual",
    "detected",
    "whoop",
    "health_import",
    "lifting",
    "activity_file",
  ]),
  duration_min: nullableNumber,
  avg_hr_bpm: nullableNumber,
  strain_score: nullableNumber,
})

/** Bounded Training Context, with separate logged and NOOP Workouts for every calendar day. */
export const trainingContext = z.object({
  from_day: z.iso.date(),
  to_day: z.iso.date(),
  time_zone: z.string(),
  noop: z.object({
    imported_device_id: z.string(),
    computed_device_id: z.string(),
    last_push_at: z.object({
      recovery: timestamp.nullable(),
      workouts: timestamp.nullable(),
    }),
  }),
  days: z
    .object({
      day: z.iso.date(),
      workouts: z
        .object({
          id: z.number().int(),
          name: z.string().nullable(),
          exercises: z
            .object({
              exercise: z.string(),
              sets: z.number().int(),
              best_reps: nullableNumber,
              best_added_weight_kg: nullableNumber,
              best_hold_duration_s: nullableNumber,
              best_rpe: nullableNumber,
            })
            .array(),
        })
        .array(),
      noop_workouts: noopSummary.array().max(24),
      body_weight: measurement,
      sleep_duration: measurement,
      resting_hr: measurement,
      hrv_rmssd: measurement,
      noop: z.object({
        recovery: z.object({ freshness, coverage: nightCoverage }),
        workouts: z.object({ freshness, coverage: workoutCoverage }),
      }),
    })
    .array()
    .max(90),
})

/** Training Context as returned by Axum. */
export type TrainingContext = z.infer<typeof trainingContext>

const unavailableReason = z.enum([
  "hr_max_not_configured",
  "no_strap_samples",
  "not_sustained",
  "no_end_samples",
])

const recoveryMark = z
  .object({ hr_bpm: z.number(), drop_bpm: z.number() })
  .nullable()

/** One selected NOOP Workout's bounded strap series, zones and Heart Rate Recovery. */
export const noopWorkoutDetail = noopSummary.omit({ avg_hr_bpm: true }).extend({
  distance_m: nullableNumber,
  energy_kcal: nullableNumber,
  avg_pace_s_per_km: nullableNumber,
  hr: z.object({
    basis: z.enum(["strap_samples", "workout_row"]).nullable(),
    avg_bpm: nullableNumber,
    min_bpm: nullableNumber,
    max_bpm: nullableNumber,
  }),
  hr_series: z
    .object({
      bucket_s: z.number().positive(),
      points: z
        .object({ t_s: z.number().nonnegative(), bpm: z.number() })
        .array()
        .max(300),
    })
    .nullable(),
  hr_zones: z
    .object({
      hrmax_used: z.number(),
      below_zone_min: z.number(),
      zones: z
        .object({
          zone: z.number(),
          lower_bpm: z.number(),
          upper_bpm: z.number(),
          minutes: z.number(),
          percent: z.number(),
        })
        .array()
        .length(5),
    })
    .nullable(),
  hr_recovery: z
    .object({
      end_hr_bpm: z.number(),
      at_1_min: recoveryMark,
      at_2_min: recoveryMark,
      at_5_min: recoveryMark,
    })
    .nullable(),
  unavailable: z.object({
    hr_series: unavailableReason.optional(),
    hr_zones: unavailableReason.optional(),
    hr_recovery: unavailableReason.optional(),
  }),
  noop: z.object({ ...syncFields, coverage: workoutCoverage }),
})

/** NOOP Workout Detail as returned by Axum. */
export type NoopWorkoutDetail = z.infer<typeof noopWorkoutDetail>

/** One trend metric over a bounded day range; a null value is a day with nothing recorded. */
export const metricTrend = z.object({
  metric: z.string(),
  from_day: z.iso.date(),
  to_day: z.iso.date(),
  unit: z.string(),
  daily_points: z
    .object({ day: z.iso.date(), value: nullableNumber })
    .array()
    .max(90),
})

/** Metric Trend as returned by Axum. */
export type MetricTrend = z.infer<typeof metricTrend>

/** A trend metric read beside a Recovery Day, by its Axum name. */
export type TrendMetric = "hrv" | "resting_heart_rate" | "sleep_duration"

/** Calendar arithmetic on a Training Day, not elapsed local hours across DST. */
export function shiftDay(day: string, days: number): string {
  const date = new Date(`${day}T00:00:00Z`)
  date.setUTCDate(date.getUTCDate() + days)

  return date.toISOString().slice(0, 10)
}

/** Days in a trend read, ending on the selected day. */
export const trendDays = 30

/** Selected Training Day and atomic NOOP selection, safe for URL search and server-function input. */
export const trainingSearch = z.object({
  day: z.iso.date().optional().catch(undefined),
  noop: noopSelection.optional().catch(undefined),
})

/** URL selection for the recovery view. */
export type TrainingSearch = z.infer<typeof trainingSearch>

/** Exact source/start/sport identity supplied by Axum, including colliding start times. */
export type NoopSelection = z.infer<typeof noopSelection>

/** Safe failures for a training read; upstream diagnostic text is never serialized. */
export type TrainingFailure =
  | Exclude<ConnectionState, "connected">
  | "invalid"
  | "not-found"
  | "too-large"

/** Independent reads let recovery stay useful if context or a selected detail is unavailable. */
export type TrainingRead<T> =
  | { readonly state: "available"; readonly data: T }
  | { readonly state: "unavailable"; readonly reason: TrainingFailure }

/** Selected-day recovery, the six days before it, a seven-day context, trends and optional NOOP detail. */
export type TrainingState = {
  readonly day: string
  /** The current Training Day when the read was made; later days cannot be selected. */
  readonly today: string
  readonly recovery: TrainingRead<RecoveryDay>
  /** Seven Recovery Days, oldest first, ending with the selected day's read. */
  readonly week: ReadonlyArray<{
    readonly day: string
    readonly recovery: TrainingRead<RecoveryDay>
  }>
  readonly trends: Readonly<Record<TrendMetric, TrainingRead<MetricTrend>>>
  readonly context: TrainingRead<TrainingContext>
  readonly detail: TrainingRead<NoopWorkoutDetail> | { readonly state: "none" }
}

/** Actionable, sanitized read failures. */
export const trainingFailureMessage: Record<TrainingFailure, string> = {
  "missing-configuration":
    "Connect OpenHome in Connection to read training data.",
  "invalid-configuration": "Check the private OpenHome setup in Connection.",
  "rejected-credential": "OpenHome rejected the API key. Check Connection.",
  unreachable: "OpenHome is unreachable. Refresh when the connection returns.",
  unavailable: "OpenHome could not read this data. Refresh to try again.",
  invalid:
    "OpenHome rejected the selection. Choose another day or reopen the NOOP workout from the list.",
  "not-found":
    "This NOOP workout is no longer available. Refresh and choose it from the list again.",
  "too-large":
    "This read exceeds OpenHome's data limit. Choose another day; no data was silently truncated.",
}

/** API reasons for missing detail blocks, without guessing measurements or Profile values. */
export const detailUnavailableMessage: Record<
  z.infer<typeof unavailableReason>,
  string
> = {
  hr_max_not_configured: "Set Maximum heart rate in Body to enable this read.",
  no_strap_samples:
    "No strap heart rate samples are available for this workout.",
  not_sustained:
    "The API found no sustained effort eligible for Heart Rate Recovery.",
  no_end_samples:
    "There are too few strap samples at the end to measure Heart Rate Recovery.",
}
