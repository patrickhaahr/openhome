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

const clock = new Intl.DateTimeFormat("en-GB", {
  timeZone: "Europe/Copenhagen",
  hour: "2-digit",
  minute: "2-digit",
  hourCycle: "h23",
})

function pace(secondsPerKm: number): string {
  const rounded = Math.round(secondsPerKm)

  return `${Math.floor(rounded / 60)}:${String(rounded % 60).padStart(2, "0")}`
}

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

/** One strap session: its figures, heart rate over time on its zones, zone time and recovery. */
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

  const stats: Array<readonly [string, string | null, string]> = [
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

  return (
    <article className="noop-detail">
      <header className="noop-detail-head">
        <div>
          <p className="sr-only">NOOP workout detail</p>
          <h2>{data.sport}</h2>
          <p className="noop-detail-time">
            {clock.format(new Date(data.start))}
            <span aria-hidden="true"> – </span>
            <span className="sr-only"> to </span>
            {clock.format(new Date(data.end))}
          </p>
          <p className="sr-only">
            Source: {data.source} · Origin: {data.origin.replaceAll("_", " ")} ·
            Heart rate basis: {basis(data)}
          </p>
        </div>
        <ScoreRing
          kind="effort"
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
      <div className="noop-detail-glance" data-route={route !== undefined}>
        <dl className="noop-detail-stats">
          {stats.flatMap(([label, figure, unit]) =>
            figure === null
              ? []
              : [
                  <div key={label}>
                    <dt>{label}</dt>
                    <dd>
                      {figure}
                      {unit === "" ? null : <small> {unit}</small>}
                    </dd>
                  </div>,
                ]
          )}
        </dl>
        {route === undefined ? null : <RouteMap route={route} />}
      </div>
      {data.hr_series === null ? (
        <p className="noop-detail-missing">
          <HeartbeatIcon size={18} aria-hidden="true" />
          {unavailable("hr_series")}
        </p>
      ) : (
        <HeartRateChart
          series={data.hr_series}
          zones={data.hr_zones}
          durationS={(Date.parse(data.end) - Date.parse(data.start)) / 1000}
        />
      )}
      {data.hr_zones === null ? (
        <p className="noop-detail-missing">{unavailable("hr_zones")}</p>
      ) : (
        <ZoneBar zones={data.hr_zones} />
      )}
      {data.hr_recovery === null ? (
        <p className="noop-detail-missing">{unavailable("hr_recovery")}</p>
      ) : (
        <HeartRateRecovery recovery={data.hr_recovery} />
      )}
    </article>
  )
}

type Series = NonNullable<NoopWorkoutDetail["hr_series"]>

type Zones = NonNullable<NoopWorkoutDetail["hr_zones"]>

/** Strap heart rate as a line over its zone bands; a missing bucket stays a gap. */
function HeartRateChart({
  series,
  zones,
  durationS,
}: {
  readonly series: Series
  readonly zones: Zones | null
  readonly durationS: number
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
    <figure className="hr-chart">
      <svg
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
                  className="hr-chart-zone"
                  data-zone={zone.zone}
                  x="0"
                  width={width}
                  y={y(top)}
                  height={y(bottom) - y(top)}
                />
              )
            })}
        <path className="hr-chart-line" d={line} />
      </svg>
      <figcaption>
        <span>{high}</span>
        <span>{low}</span>
      </figcaption>
      <details className="sr-only">
        <summary>Inspect observed heart rate values</summary>
        <div className="training-series-values">
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

/** Time in each zone as one bar, widest zone longest; minutes beneath. */
function ZoneBar({ zones }: { readonly zones: Zones }): React.JSX.Element {
  return (
    <div className="zone-bar">
      <p className="sr-only">
        Profile maximum: {zones.hrmax_used} beats/min · Below zone 1:{" "}
        {formatDecimal(zones.below_zone_min)} min
      </p>
      <div className="zone-bar-track" aria-hidden="true">
        {zones.zones.map((zone) =>
          zone.percent > 0 ? (
            <span
              key={zone.zone}
              data-zone={zone.zone}
              // SAFETY: CSSProperties omits custom properties; --minutes is the zone's share weight.
              style={{ "--minutes": zone.percent } as React.CSSProperties}
            />
          ) : null
        )}
      </div>
      <ol className="zone-bar-legend">
        {zones.zones.map((zone) => (
          <li key={zone.zone} data-zone={zone.zone}>
            <span className="zone-bar-name">
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

/** How fast the heart came down after the effort: the drop at one, two and five minutes. */
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
    <div className="hr-recovery">
      <p className="hr-recovery-end">
        <HeartbeatIcon size={18} weight="fill" aria-hidden="true" />
        <span className="sr-only">End heart rate: </span>
        {recovery.end_hr_bpm}
        <small> bpm</small>
      </p>
      <ol>
        {marks.map(([mark, reading]) => (
          <li key={mark} data-gap={reading === null}>
            <span className="hr-recovery-mark">{mark}</span>
            <span
              className="hr-recovery-bar"
              aria-hidden="true"
              // SAFETY: CSSProperties omits custom properties; --share is a 0..1 bar length.
              style={
                {
                  "--share": (reading?.drop_bpm ?? 0) / deepest,
                } as React.CSSProperties
              }
            />
            <span className="hr-recovery-drop">
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

/** The captured route on the shared map: a sun line from its start to its end. */
function RouteMap({ route }: { readonly route: Route }): React.JSX.Element {
  const theme = useMapTheme()
  const first = route[0] ?? [0, 0]
  const last = route.at(-1) ?? first

  // One stable object per route, so the map frames it once instead of on every render.
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
    <figure className="noop-detail-map">
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
