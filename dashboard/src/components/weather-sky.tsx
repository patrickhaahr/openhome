import { MoonIcon } from "@phosphor-icons/react/dist/csr/Moon"
import { useDaylight } from "./daylight"
import { daylight } from "@/lib/daylight"
import type { WeatherRead } from "@/lib/weather"

type Hours = Extract<WeatherRead, { state: "fresh" }>["forecast"]["hourly"]

const hourMs = 3_600_000

/** Labels every third hour, so the line carries the trend and the figures stay few. */
const labelEvery = 3

const hourTime = new Intl.DateTimeFormat("en-GB", {
  timeZone: "Europe/Copenhagen",
  hour: "2-digit",
  minute: "2-digit",
})

/** The stretches of [from, to] the sun is down, from the same model that turns the theme. */
function nights(from: number, to: number): { from: number; to: number }[] {
  const days = [from, to]
    .map((at) => daylight(new Date(at)))
    .flatMap(({ sunrise, sunset }) =>
      sunrise === null || sunset === null
        ? []
        : [{ from: sunrise.getTime(), to: sunset.getTime() }]
    )
    .sort((a, b) => a.from - b.from)

  const dark: { from: number; to: number }[] = []
  let cursor = from

  for (const day of days) {
    if (day.from > cursor)
      dark.push({ from: cursor, to: Math.min(day.from, to) })

    cursor = Math.max(cursor, day.to)
  }

  if (cursor < to) dark.push({ from: cursor, to })

  return dark.filter((night) => night.to > night.from)
}

/**
 * The next day as one horizon: temperature as a line, rain as columns beneath it, the
 * nights as darker ground and the sun on the line at this minute. Drawn for the eye only;
 * the hourly table carries every value for assistive technology.
 */
export function DaySky({ hours }: { hours: Hours }): React.JSX.Element | null {
  const { now } = useDaylight()
  const first = hours.at(0)
  const last = hours.at(-1)

  if (first === undefined || last === undefined || hours.length < 2) return null

  const start = Date.parse(first.starts_at)
  const span = Math.max(hourMs, Date.parse(last.starts_at) - start)
  const share = (at: number) => (at - start) / span

  const temperatures = hours.flatMap((hour) =>
    hour.temperature_c === null ? [] : [hour.temperature_c]
  )

  const coldest = Math.min(...temperatures)
  const warmest = Math.max(...temperatures)

  /** 0 at the top of the line's lane, 1 at the bottom. */
  const drop = (celsius: number) =>
    warmest === coldest ? 0.5 : (warmest - celsius) / (warmest - coldest)

  const points = hours.map((hour) => ({
    x: share(Date.parse(hour.starts_at)),
    y: hour.temperature_c === null ? null : drop(hour.temperature_c),
    hour,
  }))

  const path = points
    .map((point, index) =>
      point.y === null
        ? ""
        : `${index > 0 && points[index - 1]?.y !== null ? "L" : "M"}${(point.x * 100).toFixed(2)},${(point.y * 100).toFixed(2)}`
    )
    .join(" ")

  const peak = Math.max(1, ...hours.map((hour) => hour.precipitation_mm ?? 0))
  const sunX = Math.min(1, Math.max(0, share(now.getTime())))
  const before = points.filter((point) => point.x <= sunX).at(-1)
  const after = points.find((point) => point.x >= sunX)

  const sunY =
    before?.y === null || before === undefined
      ? null
      : after?.y === null || after === undefined || after.x === before.x
        ? before.y
        : before.y +
          ((after.y - before.y) * (sunX - before.x)) / (after.x - before.x)

  return (
    <figure className="wx-sky" aria-hidden="true">
      <div className="wx-sky-plot">
        {nights(start, start + span).map((night) => (
          <span
            key={night.from}
            className="wx-sky-night"
            // SAFETY: CSSProperties omits custom properties; --from and --to are 0..1 shares of the day.
            style={
              {
                "--from": share(night.from),
                "--to": share(night.to),
              } as React.CSSProperties
            }
          >
            <MoonIcon size={20} weight="fill" />
          </span>
        ))}
        <div className="wx-sky-lane">
          {/* Stretched to the lane; the stroke keeps its width and the marks are HTML. */}
          <svg viewBox="0 0 100 100" preserveAspectRatio="none">
            <path className="wx-sky-line" d={path} />
          </svg>
          {points.map((point, index) =>
            point.y === null || index % labelEvery !== 0 ? null : (
              <span
                key={point.hour.starts_at}
                className="wx-sky-temp"
                // SAFETY: CSSProperties omits custom properties; --x and --y are 0..1 shares of the lane.
                style={
                  { "--x": point.x, "--y": point.y } as React.CSSProperties
                }
              >
                {Math.round(point.hour.temperature_c ?? 0)}°
              </span>
            )
          )}
          {sunY === null ? null : (
            <span
              className="wx-sky-sun"
              // SAFETY: CSSProperties omits custom properties; --x and --y are 0..1 shares of the lane.
              style={{ "--x": sunX, "--y": sunY } as React.CSSProperties}
            />
          )}
        </div>
        <div className="wx-sky-rain">
          {points.map((point) =>
            (point.hour.precipitation_mm ?? 0) === 0 ? null : (
              <span
                key={point.hour.starts_at}
                // SAFETY: CSSProperties omits custom properties; --x is a 0..1 share of the day, --h of the lane.
                style={
                  {
                    "--x": point.x,
                    "--h": (point.hour.precipitation_mm ?? 0) / peak,
                    "--w": 1 / hours.length,
                  } as React.CSSProperties
                }
              />
            )
          )}
        </div>
      </div>
      <div className="wx-sky-hours">
        {points.map((point, index) =>
          index % labelEvery !== 0 ? null : (
            <span
              key={point.hour.starts_at}
              // SAFETY: CSSProperties omits custom properties; --x is a 0..1 share of the day.
              style={{ "--x": point.x } as React.CSSProperties}
            >
              {hourTime.format(new Date(point.hour.starts_at))}
            </span>
          )
        )}
      </div>
    </figure>
  )
}
