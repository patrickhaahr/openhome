import { MoonIcon } from "@phosphor-icons/react/dist/csr/Moon"
import { useDaylight } from "./daylight"
import { daylight } from "@/lib/daylight"
import { percent } from "./weather-figures"
import { cn } from "@/lib/utils"
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
    <figure
      className="grid flex-1 grid-rows-fill-hug gap-3.5"
      aria-hidden="true"
    >
      <div className="relative">
        {nights(start, start + span).map((night) => (
          <span
            key={night.from}
            className="absolute inset-y-0 left-(--x) flex w-(--w) justify-center rounded-lg bg-wash pt-3.5 text-muted-ink"
            // SAFETY: CSSProperties omits custom properties; --x and --w are 0..1 shares of the day, as percentages.
            style={
              {
                "--x": percent(share(night.from)),
                "--w": percent(share(night.to) - share(night.from)),
              } as React.CSSProperties
            }
          >
            <MoonIcon size={20} weight="fill" />
          </span>
        ))}
        <div className="absolute inset-x-0 top-lane-top bottom-lane-bottom">
          {/* Stretched to the lane; the stroke keeps its width and the marks are HTML. */}
          <svg
            className="size-full overflow-visible text-ink"
            viewBox="0 0 100 100"
            preserveAspectRatio="none"
          >
            <path
              d={path}
              fill="none"
              stroke="currentColor"
              strokeWidth={3}
              strokeLinecap="round"
              strokeLinejoin="round"
              vectorEffect="non-scaling-stroke"
            />
          </svg>
          {points.map((point, index) =>
            point.y === null || index % labelEvery !== 0 ? null : (
              <span
                key={point.hour.starts_at}
                className="absolute top-(--y) left-(--x) -mt-2.5 -translate-x-1/2 -translate-y-full text-figure-xs font-semibold num after:absolute after:-bottom-3.5 after:left-1/2 after:size-2 after:-translate-x-1/2 after:rounded-full after:bg-ink first-of-type:after:hidden"
                // SAFETY: CSSProperties omits custom properties; --x and --y are 0..1 shares of the lane, as percentages.
                style={
                  {
                    "--x": percent(point.x),
                    "--y": percent(point.y),
                  } as React.CSSProperties
                }
              >
                {Math.round(point.hour.temperature_c ?? 0)}°
              </span>
            )
          )}
          {sunY === null ? null : (
            <span
              className="absolute top-(--y) left-(--x) size-5 -translate-1/2 rounded-full bg-sun outline-4 outline-ground"
              // SAFETY: CSSProperties omits custom properties; --x and --y are 0..1 shares of the lane, as percentages.
              style={
                {
                  "--x": percent(sunX),
                  "--y": percent(sunY),
                } as React.CSSProperties
              }
            />
          )}
        </div>
        <div className="absolute inset-x-0 bottom-0 h-rain-lane">
          {points.map((point) =>
            (point.hour.precipitation_mm ?? 0) === 0 ? null : (
              <span
                key={point.hour.starts_at}
                className="absolute bottom-0 left-(--x) h-(--h) w-(--w) -translate-x-1/2 rounded-t-bar bg-rain"
                // SAFETY: CSSProperties omits custom properties; --x is a share of the day, --w and --h the bar's size with a floor.
                style={
                  {
                    "--x": percent(point.x),
                    "--w": `max(0.5rem, ${percent((1 / hours.length) * 0.6)})`,
                    "--h": `max(0.25rem, ${percent((point.hour.precipitation_mm ?? 0) / peak)})`,
                  } as React.CSSProperties
                }
              />
            )
          )}
        </div>
      </div>
      <div className="relative h-5 text-control font-medium text-muted-ink tabular-nums">
        {points.map((point, index) =>
          index % labelEvery !== 0 ? null : (
            <span
              key={point.hour.starts_at}
              className={cn(
                "absolute left-(--x) max-phone:even:hidden",
                index === 0 ? "text-ink" : "-translate-x-1/2"
              )}
              // SAFETY: CSSProperties omits custom properties; --x is a 0..1 share of the day, as a percentage.
              style={{ "--x": percent(point.x) } as React.CSSProperties}
            >
              {hourTime.format(new Date(point.hour.starts_at))}
            </span>
          )
        )}
      </div>
    </figure>
  )
}
