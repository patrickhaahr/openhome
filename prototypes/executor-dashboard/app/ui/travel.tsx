import { useState } from "react";
import { DemoMap } from "./map";
import { RoutePanel, TransitPanel } from "./panels";

/** One full-content map destination with separate route and transit modes. */
export function TravelPage(): React.JSX.Element {
  const [tab, setTab] = useState<"routes" | "transit">("routes");
  return (
    <div className="travel-page">
      <div className="feature-heading">
        <div>
          <h1>Routes & transit</h1>
        </div>
        <div className="segmented" aria-label="Travel mode">
          <button aria-pressed={tab === "routes"} onClick={() => setTab("routes")}>
            Routes
          </button>
          <button aria-pressed={tab === "transit"} onClick={() => setTab("transit")}>
            Transit
          </button>
        </div>
      </div>
      {tab === "routes" ? (
        <div className="full-route">
          <RoutePanel />
        </div>
      ) : (
        <TransitView />
      )}
    </div>
  );
}

function TransitView(): React.JSX.Element {
  const [mode, setMode] = useState<"2d" | "3d">("2d");
  const [from, setFrom] = useState("Demo stop");
  const [to, setTo] = useState("Nørreport");
  const [planned, setPlanned] = useState(false);
  return (
    <div className="transit-view">
      <DemoMap mode={mode} showRoute={false} />
      <div className="transit-overlay">
        <div className="segmented" aria-label="Transit map view">
          {(["2d", "3d"] as const).map((value) => (
            <button key={value} aria-pressed={mode === value} onClick={() => setMode(value)}>
              {value.toUpperCase()}
            </button>
          ))}
        </div>
        <section className="card journey-card">
          <span className="eyebrow">Demo</span>
          <h2>Journey planner</h2>
          <form
            onSubmit={(event) => {
              event.preventDefault();
              setPlanned(true);
            }}
          >
            <label>
              From
              <input
                value={from}
                required
                onChange={(event) => {
                  setFrom(event.target.value);
                  setPlanned(false);
                }}
              />
            </label>
            <label>
              To
              <input
                value={to}
                required
                onChange={(event) => {
                  setTo(event.target.value);
                  setPlanned(false);
                }}
              />
            </label>
            <button className="primary-button" type="submit">
              Preview journey UI
            </button>
          </form>
          {planned ? (
            <p className="journey-result" role="status">
              {from} → {to}
              <br />A 5C → walking connection would appear here. This is a layout preview, not a
              calculated journey.
            </p>
          ) : (
            <p className="panel-note">Rejseplan planning and Buster positions are not connected.</p>
          )}
        </section>
        <TransitPanel />
      </div>
      <span className="transit-map-status">Public basemap · no live vehicles connected</span>
    </div>
  );
}
