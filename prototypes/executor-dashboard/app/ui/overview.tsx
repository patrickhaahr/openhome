import { Clock } from "./clock";
import { ControlsPanel } from "./controls";
import type { ControlsProps } from "./controls";
import { HomeWeather } from "./weather";
import { ProgressPanel, TransitPanel } from "./panels";
import { DemoMap } from "./map";
import type { Page } from "./navigation";

type Props = ControlsProps & {
  readonly now: Date;
  readonly navigate: (page: Page) => void;
};
const weekdayFormat = new Intl.DateTimeFormat("en-GB", { weekday: "long" });
const dateFormat = new Intl.DateTimeFormat("en-GB", {
  day: "numeric",
  month: "long",
  year: "numeric",
});

/** A glanceable household overview; sidebar navigation owns the full feature pages. */
export function Overview({ now, navigate, controls, control }: Props): React.JSX.Element {
  return (
    <>
      <h1 className="sr-only">Home</h1>
      <div className="home-masthead">
        <section className="home-time" aria-label="Time and date">
          <Clock now={now} className="home-clock" />
          <div className="home-date">
            <strong>{weekdayFormat.format(now)}</strong>
            <time dateTime={now.toLocaleDateString("en-CA")}>{dateFormat.format(now)}</time>
          </div>
        </section>
        <HomeWeather now={now} />
        <button
          className="home-agent-avatar"
          aria-label="Open agent page"
          onClick={() => navigate("agent")}
        >
          <span className="orb-wrap" aria-hidden="true">
            <span className="orb-orbit" />
            <span className="orb">
              <i />
              <i />
              <i />
            </span>
          </span>
          <span>Agent</span>
        </button>
      </div>
      <div className="home-features">
        <div className="home-controls-preview">
          <ControlsPanel controls={controls} control={control} />
        </div>
        <section className="card home-routes-preview">
          <div className="card-top">
            <h2>Routes & transit</h2>
            <span className="small-badge">Sample route</span>
          </div>
          <DemoMap mode="2d" overview />
        </section>
        <div className="home-workout-preview">
          <ProgressPanel />
        </div>
        <div className="home-transit-preview">
          <TransitPanel overview />
        </div>
      </div>
    </>
  );
}
