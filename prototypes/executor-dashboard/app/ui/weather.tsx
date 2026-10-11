import { useState } from "react";
import { CloudSunIcon } from "@phosphor-icons/react/dist/csr/CloudSun";
import { CloudIcon } from "@phosphor-icons/react/dist/csr/Cloud";
import { CloudRainIcon } from "@phosphor-icons/react/dist/csr/CloudRain";
import { SunIcon } from "@phosphor-icons/react/dist/csr/Sun";
import { CaretUpIcon } from "@phosphor-icons/react/dist/csr/CaretUp";
import { ArrowUpRightIcon } from "@phosphor-icons/react/dist/csr/ArrowUpRight";

const forecast = [
  { high: 16, low: 10, rain: "10%", icon: CloudSunIcon, condition: "Partly cloudy" },
  { high: 14, low: 9, rain: "45%", icon: CloudIcon, condition: "Cloudy" },
  { high: 13, low: 8, rain: "65%", icon: CloudRainIcon, condition: "Rain" },
  { high: 15, low: 10, rain: "20%", icon: SunIcon, condition: "Sunny" },
  { high: 14, low: 9, rain: "30%", icon: CloudSunIcon, condition: "Partly cloudy" },
  { high: 12, low: 7, rain: "55%", icon: CloudRainIcon, condition: "Rain" },
  { high: 13, low: 8, rain: "25%", icon: CloudSunIcon, condition: "Partly cloudy" },
] as const;
const dayFormat = new Intl.DateTimeFormat("en-GB", { weekday: "short" });

/** Seven illustrative forecasts, dated relative to the device's current day. */
export function ForecastDays({ now }: { readonly now: Date }): React.JSX.Element {
  return (
    <div className="forecast-days" aria-label="Today and six days ahead">
      {forecast.map((day, index) => {
        const date = new Date(now);
        date.setDate(date.getDate() + index);
        return (
          <div key={index}>
            <strong>{index === 0 ? "Today" : dayFormat.format(date)}</strong>
            <span role="img" aria-label={day.condition}>
              <day.icon size={24} />
            </span>
            <b>
              {day.high}° <small>{day.low}°</small>
            </b>
            <p>{day.rain} rain</p>
          </div>
        );
      })}
    </div>
  );
}

export function HomeWeather({ now }: { readonly now: Date }): React.JSX.Element {
  return (
    <section className="home-forecast" aria-label="Weather demo">
      <div className="home-forecast-current">
        <CloudSunIcon size={38} />
        <strong>14°</strong>
        <div>
          <h2>Partly cloudy</h2>
          <p>High 16° · Low 10°</p>
        </div>
        <span className="eyebrow">Weather · sample</span>
      </div>
      <ForecastDays now={now} />
    </section>
  );
}

export function WeatherPanel(): React.JSX.Element {
  const [extended, setExtended] = useState(false);
  return (
    <section className="card weather-card">
      <div className="card-top">
        <span className="eyebrow">Weather · sample</span>
        <span className="weather-symbol">
          <CloudSunIcon size={32} />
        </span>
      </div>
      <div className="weather-main">
        <strong>14°</strong>
        <div>
          <h2>Partly cloudy</h2>
          <p>16° / 10°</p>
        </div>
      </div>
      <div className="weather-details">
        <span>
          Feels like <b>12°</b>
        </span>
        <span>
          Wind <b>3 m/s</b>
        </span>
        <span>
          Rain <b>10%</b>
        </span>
      </div>
      <button
        className="text-button"
        aria-expanded={extended}
        onClick={() => setExtended(!extended)}
      >
        {extended ? "Hide hourly outlook" : "Hourly outlook"}
        {extended ? <CaretUpIcon size={16} /> : <ArrowUpRightIcon size={16} />}
      </button>
      {extended ? (
        <div className="hourly">
          {["10:00 · 14°", "12:00 · 16°", "14:00 · 15°", "16:00 · 13°"].map((hour) => (
            <span key={hour}>{hour}</span>
          ))}
        </div>
      ) : null}
    </section>
  );
}
