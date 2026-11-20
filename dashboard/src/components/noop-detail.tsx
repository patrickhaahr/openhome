import { useMemo } from "react"
import { HeartbeatIcon } from "@phosphor-icons/react/dist/csr/Heartbeat"
import { ScoreRing } from "@/components/score-ring"
import { useMapTheme } from "@/components/daylight"
import { SharedMap } from "@/components/shared-map"
import { formatDecimal, formatWhole } from "@/lib/number-format"
import { ringShare, shortDuration } from "@/lib/recovery-score"
import {
  detailUnavailableMessage,
  type NoopWorkoutDetail,
} from "@/lib/training"

import { clockTime, pace } from "@/lib/session-format"
import { capsLabel } from "@/components/training-shared"

export type StrapFit = "detail" | "pane" | "alone"

function basis(data: NoopWorkoutDetail): string {
  switch (data.hr.basis) {
    case "strap_samples":
      return `strap samples (${data.noop.imported_device_id})`
    case "workout_row":
      return `stored workout row (${data.source}); unreconciled with the strap`
    case null:
      return "Not available"
  }
}

export function noopStats(
  data: NoopWorkoutDetail
): Array<readonly [string, string | null, string]> {
  return [
    [
      "Duration",
      data.duration_min === null ? null : shortDuration(data.duration_min),
      "",
    ],
    [
      "Average HR",
      data.hr.avg_bpm === null ? null : String(Math.round(data.hr.avg_bpm)),
      "bpm",
    ],
    [
      "Maximum HR",
      data.hr.max_bpm === null ? null : String(Math.round(data.hr.max_bpm)),
      "bpm",
    ],
    [
      "Energy",
      data.energy_kcal === null ? null : String(Math.round(data.energy_kcal)),
      "kcal",
    ],
    [
      "Distance",
      data.distance_m === null ? null : formatDecimal(data.distance_m / 1000),
      "km",
    ],
    [
      "Average pace",
      data.avg_pace_s_per_km === null ? null : pace(data.avg_pace_s_per_km),
      "/km",
    ],
    ["Steps", data.steps === null ? null : formatWhole(data.steps), ""],
  ]
}

const statLooks = {
  detail: {
    list: "flex flex-wrap gap-x-detail-stats gap-y-4",
    figure: "font-semibold tracking-workout-figure",
  },
  route: {
    list: "grid grid-cols-2 content-center gap-x-detail-stats gap-y-4",
    figure: "font-semibold tracking-workout-figure",
  },
  session: {
    list: "grid min-w-0 flex-1 grid-cols-stats gap-x-4 gap-y-3.5 max-[700px]:w-full max-[700px]:grid-cols-2",
    figure: "leading-workout-heading font-bold tracking-title",
  },
} as const

export function StatList({
  stats,
  look,
}: {
  readonly stats: ReadonlyArray<readonly [string, string | null, string]>
  readonly look: keyof typeof statLooks
}): React.JSX.Element {
  return (
    <dl className={statLooks[look].list}>
      {stats.flatMap(([label, figure, unit]) =>
        figure === null
          ? []
          : [
              <div key={label} className="grid gap-0.5">
                <dt className={capsLabel}>{label}</dt>
                <dd
                  className={`text-figure-sm tabular-nums ${statLooks[look].figure}`}
                >
                  {figure}
                  {unit === "" ? null : (
                    <small className="text-small font-medium tracking-normal text-muted-ink">
                      {" "}
                      {unit}
                    </small>
                  )}
                </dd>
              </div>,
            ]
      )}
    </dl>
  )
}

export function NoopDetail({
  data,
}: {
  readonly data: NoopWorkoutDetail
}): React.JSX.Element {
  const unavailable = (block: keyof NoopWorkoutDetail["unavailable"]) => {
    const code = data.unavailable[block]

    return code === undefined
      ? "Not available."
      : detailUnavailableMessage[code]
  }

  const route =
    data.route !== undefined && data.route.length > 1 ? data.route : undefined

  return (
    <article
      data-slot="noop-detail"
      className="grid gap-6 rounded-panel bg-tile p-plate"
    >
      <header className="flex items-center gap-6 pr-14">
        <div className="flex-1">
          <p className="sr-only">NOOP workout detail</p>
          <h2 className="text-lead font-semibold tracking-title">
            {data.sport}
          </h2>
          <p className="text-muted-ink tabular-nums">
            {clockTime(data.start)}
            <span aria-hidden="true"> – </span>
            <span className="sr-only"> to </span>
            {clockTime(data.end)}
          </p>
          <p className="sr-only">
            Source: {data.source} · Origin: {data.origin.replaceAll("_", " ")} ·
            Heart rate basis: {basis(data)}
          </p>
        </div>
        <ScoreRing
          kind="effort"
          size="detail"
          share={ringShare(data.strain_score, 100)}
          band={data.strain_score === null ? "none" : "fair"}
          figure={
            data.strain_score === null
              ? null
              : String(Math.round(data.strain_score))
          }
          label="Effort"
          spoken={
            data.strain_score === null
              ? "Not available"
              : `strain ${Math.round(data.strain_score)} of 100`
          }
        />
      </header>
      <div
        className={`grid gap-5 ${route === undefined ? "" : "grid-cols-route items-stretch max-[900px]:grid-cols-1"}`}
      >
        <StatList
          look={route === undefined ? "detail" : "route"}
          stats={noopStats(data)}
        />
        {route === undefined ? null : <RouteMap route={route} fit="detail" />}
      </div>
      {data.hr_series === null ? (
        <p className={missingLine}>
          <HeartbeatIcon size={18} aria-hidden="true" />
          {unavailable("hr_series")}
        </p>
      ) : (
        <HeartRateChart
          series={data.hr_series}
          zones={data.hr_zones}
          durationS={(Date.parse(data.end) - Date.parse(data.start)) / 1000}
          fit="detail"
        />
      )}
      {data.hr_zones === null ? (
        <p className={missingLine}>{unavailable("hr_zones")}</p>
      ) : (
        <ZoneBar zones={data.hr_zones} fit="detail" />
      )}
      {data.hr_recovery === null ? (
        <p className={missingLine}>{unavailable("hr_recovery")}</p>
      ) : (
        <HeartRateRecovery recovery={data.hr_recovery} />
      )}
    </article>
  )
}

const missingLine = "flex items-center gap-2 text-control text-muted-ink"

export type Series = NonNullable<NoopWorkoutDetail["hr_series"]>

export type Zones = NonNullable<NoopWorkoutDetail["hr_zones"]>

const traceLooks: Record<StrapFit, { figure: string; plot: string }> = {
  detail: { figure: "relative", plot: "h-hr-chart" },
  pane: {
    figure: "relative min-h-32",
    plot: "h-full min-h-32 max-[860px]:h-36",
  },
  alone: { figure: "relative min-h-32", plot: "h-36 min-h-32" },
}

export function HeartRateChart({
  series,
  zones,
  durationS,
  fit,
}: {
  readonly series: Series
  readonly zones: Zones | null
  readonly durationS: number
  readonly fit: StrapFit
}): React.JSX.Element {
  const width = 600
  const height = 180
  const bpm = series.points.map((point) => point.bpm)
  const low = Math.max(0, Math.floor((Math.min(...bpm) - 10) / 10) * 10)
  const high = Math.ceil((Math.max(...bpm) + 10) / 10) * 10
  const span = Math.max(1, durationS)
  const x = (seconds: number) => (seconds / span) * width

  const y = (value: number) =>
    height - ((value - low) / Math.max(1, high - low)) * height

  let line = ""
  let previous: number | undefined

  for (const point of series.points) {
    const joined =
      previous !== undefined && point.t_s - previous <= series.bucket_s * 1.5

    line += `${joined ? "L" : "M"}${x(point.t_s).toFixed(1)} ${y(point.bpm).toFixed(1)}`
    previous = point.t_s
  }

  return (
    <figure className={traceLooks[fit].figure}>
      <svg
        className={`block w-full overflow-hidden rounded-plate bg-wash ${traceLooks[fit].plot}`}
        viewBox={`0 0 ${width} ${height}`}
        preserveAspectRatio="none"
        role="img"
        aria-label="Strap heart rate samples in beats per minute; missing buckets remain gaps"
      >
        {zones === null
          ? null
          : zones.zones.map((zone) => {
              const top = Math.min(
                high,
                zone.zone === 5 ? high : zone.upper_bpm
              )

              const bottom = Math.max(low, zone.lower_bpm)

              return top <= bottom ? null : (
                <rect
                  key={zone.zone}
                  className="fill-zone opacity-10"
                  data-zone={zone.zone}
                  x="0"
                  width={width}
                  y={y(top)}
                  height={y(bottom) - y(top)}
                />
              )
            })}
        <path
          className="stroke-poor"
          d={line}
          fill="none"
          strokeWidth={2.5}
          strokeLinejoin="round"
          strokeLinecap="round"
          vectorEffect="non-scaling-stroke"
        />
      </svg>
      <figcaption className="pointer-events-none absolute inset-y-2 left-3 flex flex-col justify-between text-caption text-muted-ink tabular-nums">
        <span>{high}</span>
        <span>{low}</span>
      </figcaption>
      <details className="sr-only">
        <summary>Inspect observed heart rate values</summary>
        <div
          data-slot="training-series-values"
          className="max-h-60 overflow-auto tabular-nums"
        >
          {series.points.map((point) => (
            <p key={point.t_s}>
              +{point.t_s} s: {point.bpm} beats/min
            </p>
          ))}
        </div>
      </details>
    </figure>
  )
}

export function ZoneBar({
  zones,
  fit,
}: {
  readonly zones: Zones
  readonly fit: StrapFit
}): React.JSX.Element {
  return (
    <div className="grid gap-3">
      <p className="sr-only">
        Profile maximum: {zones.hrmax_used} beats/min · Below zone 1:{" "}
        {formatDecimal(zones.below_zone_min)} min
      </p>
      <div
        className="flex h-4 gap-0.75 overflow-hidden rounded-full bg-wash"
        aria-hidden="true"
      >
        {zones.zones.map((zone) =>
          zone.percent > 0 ? (
            <span
              key={zone.zone}
              data-zone={zone.zone}
              className="min-w-1 flex-(--minutes) bg-zone"
              // SAFETY: CSSProperties omits custom properties; --minutes is the zone's share weight.
              style={{ "--minutes": zone.percent } as React.CSSProperties}
            />
          ) : null
        )}
      </div>
      <ol className="grid grid-cols-5 gap-2 tabular-nums">
        {zones.zones.map((zone) => (
          <li
            key={zone.zone}
            data-zone={zone.zone}
            className={`grid gap-0.5 text-large font-semibold ${fit === "detail" ? "max-[700px]:text-control" : ""}`}
          >
            <span className="inline-flex items-center gap-1.5 text-caption font-bold text-muted-ink before:size-2 before:rounded-full before:bg-zone">
              <span className="sr-only">Zone </span>Z{zone.zone}
              <span className="sr-only">
                {" "}
                · {zone.lower_bpm}–{zone.upper_bpm}
                {zone.zone === 5 ? "+" : ""} beats/min
              </span>
            </span>
            <meter
              className="sr-only"
              aria-label={`Zone ${zone.zone} share`}
              min={0}
              max={100}
              value={zone.percent}
            />
            <span>{shortDuration(zone.minutes)}</span>
          </li>
        ))}
      </ol>
    </div>
  )
}

function HeartRateRecovery({
  recovery,
}: {
  readonly recovery: NonNullable<NoopWorkoutDetail["hr_recovery"]>
}): React.JSX.Element {
  const marks = [
    ["1 min", recovery.at_1_min],
    ["2 min", recovery.at_2_min],
    ["5 min", recovery.at_5_min],
  ] as const

  const deepest = Math.max(
    1,
    ...marks.map(([, reading]) => reading?.drop_bpm ?? 0)
  )

  return (
    <div className="grid grid-cols-lead items-center gap-6">
      <p className="inline-flex items-center gap-1.5 text-figure-sm font-semibold tracking-workout-figure tabular-nums">
        <HeartbeatIcon
          className="text-poor"
          size={18}
          weight="fill"
          aria-hidden="true"
        />
        <span className="sr-only">End heart rate: </span>
        {recovery.end_hr_bpm}
        <small className="text-small tracking-normal text-muted-ink">
          {" "}
          bpm
        </small>
      </p>
      <ol className="grid gap-1.5">
        {marks.map(([mark, reading]) => (
          <li
            key={mark}
            data-gap={reading === null}
            className="grid grid-cols-recovery items-center gap-3 tabular-nums data-[gap=true]:gap-dash-5.5"
          >
            <span className="text-label text-muted-ink">{mark}</span>
            <span
              className="h-2.5 w-share min-w-0.5 rounded-full bg-good"
              aria-hidden="true"
              // SAFETY: CSSProperties omits custom properties; --share is a 0..1 bar length.
              style={
                {
                  "--share": (reading?.drop_bpm ?? 0) / deepest,
                } as React.CSSProperties
              }
            />
            <span className="text-right font-bold">
              {reading === null ? (
                <span className="sr-only">
                  Too few strap samples; not interpolated.
                </span>
              ) : (
                <>
                  <span aria-hidden="true">−</span>
                  <span className="sr-only">
                    {reading.hr_bpm} beats/min · Drop:{" "}
                  </span>
                  {reading.drop_bpm}
                </>
              )}
            </span>
          </li>
        ))}
      </ol>
    </div>
  )
}

type Route = NonNullable<NoopWorkoutDetail["route"]>

export function RouteMap({
  route,
  fit,
}: {
  readonly route: Route
  readonly fit: "detail" | "pane"
}): React.JSX.Element {
  const theme = useMapTheme()
  const first = route[0] ?? [0, 0]
  const last = route.at(-1) ?? first

  const routes = useMemo(
    () => ({
      lines: [
        {
          type: "LineString" as const,
          coordinates: route.map(
            ([latitude, longitude]): readonly [number, number] => [
              longitude,
              latitude,
            ]
          ),
        },
      ],
      focused: 0,
      endpoint: [last[1], last[0]] as const,
      framing: "even" as const,
    }),
    [route, last]
  )

  return (
    <figure
      className={`grid overflow-hidden rounded-plate bg-wash ${fit === "detail" ? "min-h-88 max-[900px]:min-h-72" : "h-full min-h-80 max-[860px]:min-h-64"}`}
    >
      <SharedMap
        key={`${first[0]},${first[1]}`}
        origin={{ latitude: first[0], longitude: first[1], source: "private" }}
        routes={routes}
        theme={theme}
        controls={false}
      />
      <figcaption className="sr-only">
        Route map, {route.length} points from start to finish
      </figcaption>
    </figure>
  )
}
