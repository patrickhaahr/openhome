import { createElement } from "react"
import { CloudIcon } from "@phosphor-icons/react/dist/csr/Cloud"
import { CloudFogIcon } from "@phosphor-icons/react/dist/csr/CloudFog"
import { CloudLightningIcon } from "@phosphor-icons/react/dist/csr/CloudLightning"
import { CloudRainIcon } from "@phosphor-icons/react/dist/csr/CloudRain"
import { CloudSnowIcon } from "@phosphor-icons/react/dist/csr/CloudSnow"
import { CloudSunIcon } from "@phosphor-icons/react/dist/csr/CloudSun"
import { QuestionIcon } from "@phosphor-icons/react/dist/csr/Question"
import { SunIcon } from "@phosphor-icons/react/dist/csr/Sun"
import type { Icon } from "@phosphor-icons/react/dist/lib/types"
import type { WeatherRead } from "@/lib/weather"

export type Days = Extract<WeatherRead, { state: "fresh" }>["forecast"]["days"]

const number = new Intl.NumberFormat("en-GB", { maximumFractionDigits: 1 })

const weekday = new Intl.DateTimeFormat("en-GB", {
  timeZone: "Europe/Copenhagen",
  weekday: "short",
})

/** A reading with its unit as words for assistive technology, or "Unavailable". */
export function value(amount: number | null | undefined, unit: string): string {
  return amount === null || amount === undefined
    ? "Unavailable"
    : `${number.format(amount)} ${unit}`
}

/** A figure with its unit set smaller; the text still reads "12.4 °C". */
export function Measure({
  amount,
  unit,
}: {
  amount: number | null | undefined
  unit: string
}): React.JSX.Element {
  if (amount === null || amount === undefined)
    return (
      <span data-gap="true" className="gap-dash-4">
        Unavailable
      </span>
    )

  return (
    <>
      {number.format(amount)}{" "}
      <span className="align-baseline text-unit tracking-normal">{unit}</span>
    </>
  )
}

/** WMO code ranges and their glyphs; unknown codes never draw a clear sky. */
const glyphs: readonly (readonly [from: number, to: number, glyph: Icon])[] = [
  [0, 1, SunIcon],
  [2, 2, CloudSunIcon],
  [3, 3, CloudIcon],
  [45, 45, CloudFogIcon],
  [48, 48, CloudFogIcon],
  [51, 67, CloudRainIcon],
  [71, 77, CloudSnowIcon],
  [80, 82, CloudRainIcon],
  [85, 86, CloudSnowIcon],
  [95, 99, CloudLightningIcon],
]

/** A WMO code as a glyph. */
export function WeatherGlyph({
  code,
  size,
  className,
}: {
  code: number | null
  size: number
  className?: string
}): React.JSX.Element {
  const match = glyphs.find(
    ([from, to]) => code !== null && code >= from && code <= to
  )

  return createElement(match?.[2] ?? QuestionIcon, {
    size,
    className,
    "aria-hidden": true,
  })
}

/** A whole-degree reading, or a dash when the forecast has none. */
export function degrees(celsius: number | null | undefined): string {
  return celsius === null || celsius === undefined
    ? "–"
    : `${Math.round(celsius)}°`
}

export function weekdayName(date: string): string {
  return weekday.format(new Date(`${date}T12:00:00Z`)).replace(".", "")
}

/** A 0..1 share as a percentage, for the custom properties `marks` sets. */
export function percent(share: number): string {
  return `${share * 100}%`
}
