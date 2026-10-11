import { useState } from "react";
import type { Icon } from "@phosphor-icons/react";
import { ImagesIcon } from "@phosphor-icons/react/dist/csr/Images";
import { CalendarDotsIcon } from "@phosphor-icons/react/dist/csr/CalendarDots";
import { AgentPanel, ProgressPanel, WishlistPanel } from "./panels";
import type { PanelProps } from "./panels";
import { ControlsPanel } from "./controls";
import type { ControlsProps } from "./controls";
import { ForecastDays, WeatherPanel } from "./weather";
import type { Page } from "./navigation";
import { Overview } from "./overview";
import { TravelPage } from "./travel";
import { WatchlistPage } from "./watchlist";
import type { DemoWatch } from "./watchlist";

type Props = PanelProps &
  ControlsProps & {
    readonly now: Date;
    readonly page: Page;
    readonly navigate: (page: Page) => void;
    readonly watches: ReadonlyArray<DemoWatch>;
    readonly updateWatches: (entries: ReadonlyArray<DemoWatch>) => void;
  };

/** Feature pages share the navigation shell; each owns the content area. */
export function FeaturePage(props: Props): React.JSX.Element {
  switch (props.page) {
    case "home":
      return <Overview {...props} />;
    case "travel":
      return <TravelPage />;
    case "weather":
      return <WeatherPage now={props.now} />;
    case "controls":
      return (
        <div className="feature-page controls-page">
          <h1 className="sr-only">Home controls</h1>
          <ControlsPanel controls={props.controls} control={props.control} />
        </div>
      );
    case "workout":
      return (
        <div className="feature-page workout-page">
          <h1 className="sr-only">Workout</h1>
          <ProgressPanel />
        </div>
      );
    case "wishlist":
      return (
        <div className="feature-page wishlist-page">
          <PageHeading title="Wishlist" subtitle="Demo" />
          <WishlistPanel {...props} />
        </div>
      );
    case "watchlist":
      return <WatchlistPage entries={props.watches} update={props.updateWatches} />;
    case "agent":
      return <AgentPage {...props} />;
    case "photos":
      return (
        <ConnectionPage
          title="Photos"
          provider="Immich"
          icon={ImagesIcon}
          description="Albums"
          status="Not connected"
        />
      );
    case "calendar":
      return (
        <ConnectionPage
          title="Calendar"
          provider="Proton Calendar"
          icon={CalendarDotsIcon}
          description="Agenda · week · month"
          status="Backlog"
        />
      );
  }
}

function PageHeading({
  title,
  subtitle,
}: {
  readonly title: string;
  readonly subtitle: string;
}): React.JSX.Element {
  return (
    <div className="feature-heading">
      <div>
        <h1>{title}</h1>
        <p>{subtitle}</p>
      </div>
    </div>
  );
}

function WeatherPage({ now }: { readonly now: Date }): React.JSX.Element {
  return (
    <div className="feature-page weather-page">
      <PageHeading title="Weather" subtitle="Demo forecast" />
      <WeatherPanel />
      <section className="card forecast-card">
        <span className="eyebrow">The next seven days · sample</span>
        <ForecastDays now={now} />
      </section>
    </div>
  );
}

function AgentPage(props: PanelProps): React.JSX.Element {
  const [session, setSession] = useState(0);
  return (
    <div className="feature-page dedicated-agent">
      <div className="feature-heading">
        <div>
          <h1>Agent</h1>
        </div>
        <button className="secondary-button" onClick={() => setSession((value) => value + 1)}>
          Reset demo conversation
        </button>
      </div>
      <AgentPanel key={session} {...props} />
      <p className="feature-note">Demo · Hermes and voice not connected</p>
    </div>
  );
}

function ConnectionPage({
  title,
  provider,
  icon: Icon,
  description,
  status,
}: {
  readonly title: string;
  readonly provider: string;
  readonly icon: Icon;
  readonly description: string;
  readonly status: string;
}): React.JSX.Element {
  return (
    <div className="feature-page connection-page">
      <PageHeading title={title} subtitle={provider} />
      <section className="card empty-integration">
        <span className="integration-icon" aria-hidden="true">
          <Icon size={56} />
        </span>
        <span className="small-badge">Not connected</span>
        <h2>{provider}</h2>
        <p>{description}</p>
        <p className="integration-status" role="status">
          {status}
        </p>
      </section>
    </div>
  );
}
