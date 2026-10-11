import { useState } from "react";
import { ArrowUpRightIcon } from "@phosphor-icons/react/dist/csr/ArrowUpRight";
import { CaretUpIcon } from "@phosphor-icons/react/dist/csr/CaretUp";
import { CheckIcon } from "@phosphor-icons/react/dist/csr/Check";
import { HeartIcon } from "@phosphor-icons/react/dist/csr/Heart";
import { DemoMap } from "./map";
import { demoMembers } from "./household";
import type { DemoMemberId } from "./household";

/** A shared, immutable set of demo wishes returned by the Executor tool. */
export type Wishes = ReadonlyArray<{
  readonly id: string;
  readonly owner: string;
  readonly title: string;
}>;
/** Shared wishlist tools for dedicated dashboard pages. */
export type PanelProps = {
  readonly wishes: Wishes;
  readonly busy: boolean;
  readonly message: string;
  readonly addWish: (owner: DemoMemberId, title: string) => void;
};

/** Route-preview controls and the real public basemap. */
export function RoutePanel(): React.JSX.Element {
  const [mode, setMode] = useState<"2d" | "3d">("2d");
  const [distance, setDistance] = useState("5 km");
  const [activity, setActivity] = useState("Walk");
  const [selected, setSelected] = useState(false);
  return (
    <section className="card route-card">
      <div className="card-top">
        <div>
          <h2>Routes</h2>
        </div>
        <span className="small-badge">Map preview</span>
      </div>
      <div className="route-controls">
        <div className="segmented" aria-label="Activity">
          {["Walk", "Run"].map((value) => (
            <button
              key={value}
              aria-pressed={activity === value}
              onClick={() => {
                setActivity(value);
                setSelected(false);
              }}
            >
              {value}
            </button>
          ))}
        </div>
        <label className="distance-label">
          <span className="sr-only">Route target</span>
          <input
            aria-label="Route target"
            value={distance}
            onChange={(event) => {
              setDistance(event.target.value);
              setSelected(false);
            }}
          />
        </label>
        <div className="segmented" aria-label="Map view">
          {(["2d", "3d"] as const).map((value) => (
            <button key={value} aria-pressed={mode === value} onClick={() => setMode(value)}>
              {value.toUpperCase()}
            </button>
          ))}
        </div>
      </div>
      <DemoMap mode={mode} />
      <div className="route-footer">
        <div>
          <strong>The lakes, the long way</strong>
          <span>
            {activity} · target {distance || "not set"} · sample geometry
          </span>
        </div>
        <button className="primary-button" onClick={() => setSelected(!selected)}>
          {selected ? (
            <>
              Selected <CheckIcon size={16} />
            </>
          ) : (
            "Select preview"
          )}
        </button>
      </div>
      <p className="panel-note" role="status">
        {selected
          ? "Preview selected. Production route generation and phone sync are not connected."
          : "Explore the map. This sample loop does not change when the target changes."}
      </p>
    </section>
  );
}

/** Wishlist form writes through a real Executor mutation to isolated demo storage. */
export function WishlistPanel({ wishes, busy, message, addWish }: PanelProps): React.JSX.Element {
  return (
    <>
      <div className="household-lists">
        {demoMembers.map((member) => (
          <WishlistGroup
            key={member.id}
            member={member}
            wishes={wishes}
            busy={busy}
            addWish={addWish}
          />
        ))}
      </div>
      <p className="panel-note" role="status">
        {message || "Demo storage"}
      </p>
    </>
  );
}

function WishlistGroup({
  member,
  wishes,
  busy,
  addWish,
}: {
  readonly member: (typeof demoMembers)[number];
  readonly wishes: Wishes;
  readonly busy: boolean;
  readonly addWish: PanelProps["addWish"];
}): React.JSX.Element {
  const [title, setTitle] = useState("");
  const rows = wishes.filter((wish) => wish.owner === member.id);
  const fieldId = `wish-title-${member.id}`;
  return (
    <section className="card wishlist-card" aria-label={`${member.label} wishlist`}>
      <div className="card-top member-heading">
        <span className="member-icon" aria-hidden="true">
          {member.icon}
        </span>
        <h2>{member.label}</h2>
        <span className="count-badge">{rows.length}</span>
      </div>
      <ul className="wishes">
        {rows.map((wish) => (
          <li key={wish.id}>
            <span className="wish-icon" aria-hidden="true">
              <HeartIcon size={16} />
            </span>
            <span>{wish.title}</span>
          </li>
        ))}
      </ul>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          if (title.trim().length === 0 || busy) return;
          addWish(member.id, title.trim());
          setTitle("");
        }}
      >
        <label className="sr-only" htmlFor={fieldId}>
          New wishlist item for {member.label}
        </label>
        <input
          id={fieldId}
          value={title}
          maxLength={160}
          onChange={(event) => setTitle(event.target.value)}
          placeholder="Add an item…"
        />
        <button
          className="primary-button"
          disabled={busy || title.trim().length === 0}
          type="submit"
        >
          {busy ? "Saving…" : "Add wish"}
        </button>
      </form>
    </section>
  );
}

/** Demonstrate the orb states; no microphone or paid voice API is opened. */
export function AgentPanel({ addWish, busy }: PanelProps): React.JSX.Element {
  const [state, setState] = useState<"idle" | "listening" | "thinking" | "speaking">("idle");
  function nextState(): void {
    const next = {
      idle: "listening",
      listening: "thinking",
      thinking: "speaking",
      speaking: "idle",
    } as const;
    setState(next[state]);
  }
  return (
    <section className={`card agent-card agent-${state}`}>
      <div className="card-top">
        <span className="eyebrow">Agent</span>
        <span className="small-badge">Voice demo</span>
      </div>
      <div className="agent-body">
        <div className="orb-wrap" aria-hidden="true">
          <div className="orb-orbit" />
          <div className="orb">
            <i />
            <i />
            <i />
          </div>
        </div>
        <div className="agent-copy">
          <button className="text-button" onClick={nextState}>
            Try voice states <ArrowUpRightIcon size={16} />
          </button>
          <span className="agent-status" role="status">
            Demo · {state}
          </span>
        </div>
      </div>
      <button
        className="prompt-button"
        disabled={busy}
        onClick={() => addWish(demoMembers[0].id, "A comfortable backpack for day walks")}
      >
        “Add a day-walk backpack to {demoMembers[0].label}’s wishlist”{" "}
        <ArrowUpRightIcon size={16} />
      </button>
      <p className="panel-note">Demo tool</p>
    </section>
  );
}

const series = [
  { name: "Pull-ups", color: "var(--color-chart-primary)", values: [100, 106, 103, 119, 124, 131] },
  { name: "Dips", color: "var(--color-chart-secondary)", values: [100, 104, 110, 115, 125, 136] },
  { name: "Squat", color: "var(--color-chart-quaternary)", values: [100, 103, 109, 107, 116, 122] },
  {
    name: "Handstand",
    color: "var(--color-chart-tertiary)",
    values: [100, 111, 109, 117, 131, 140],
  },
];

/** Synthetic normalized measurements with independent line toggles. */
export function ProgressPanel(): React.JSX.Element {
  const [shown, setShown] = useState(series.map((line) => line.name));
  return (
    <section className="card progress-card">
      <div className="card-top">
        <div>
          <h2>Exercise progress</h2>
        </div>
        <span className="small-badge">Sample · baseline 100</span>
      </div>
      <svg
        viewBox="0 0 560 200"
        role="img"
        aria-label={`Synthetic relative progress for ${shown.join(", ") || "no selected exercises"}`}
      >
        <path d="M30 20H550 M30 90H550 M30 160H550" className="chart-grid" />
        <text x="0" y="25">
          140
        </text>
        <text x="0" y="95">
          120
        </text>
        <text x="0" y="165">
          100
        </text>
        {series
          .filter((line) => shown.includes(line.name))
          .map((line) => (
            <polyline
              key={line.name}
              points={line.values
                .map((value, index) => `${35 + index * 101},${160 - (value - 100) * 3.5}`)
                .join(" ")}
              fill="none"
              stroke={line.color}
              strokeWidth="2.5"
              strokeLinejoin="round"
            />
          ))}
      </svg>
      <div className="chart-legend">
        {series.map((line) => (
          <button
            key={line.name}
            aria-pressed={shown.includes(line.name)}
            onClick={() =>
              setShown(
                shown.includes(line.name)
                  ? shown.filter((name) => name !== line.name)
                  : [...shown, line.name],
              )
            }
          >
            <i style={{ background: line.color }} />
            {line.name}
          </button>
        ))}
      </div>
    </section>
  );
}

/** Illustrative transit information, never represented as a live Buster feed. */
export function TransitPanel({
  overview = false,
}: {
  readonly overview?: boolean;
}): React.JSX.Element {
  const [expanded, setExpanded] = useState(false);
  return (
    <section className="card transit-card">
      <div className="card-top">
        <div>
          <h2>{overview ? "Bus arrivals" : "Next departures"}</h2>
        </div>
        <span className="small-badge">Sample</span>
      </div>
      <div className="departure">
        <span className="bus-line">5C</span>
        <div>
          <strong>City centre</strong>
          <small>Public demo stop</small>
        </div>
        <b>
          3 <small>min</small>
        </b>
      </div>
      <div className="departure">
        <span className="bus-line purple">350S</span>
        <div>
          <strong>Nørreport</strong>
          <small>Public demo stop</small>
        </div>
        <b>
          8 <small>min</small>
        </b>
      </div>
      {overview ? null : (
        <button
          className="text-button"
          onClick={() => setExpanded(!expanded)}
          aria-expanded={expanded}
        >
          {expanded ? "Hide integration details" : "Transit integrations"}
          {expanded ? <CaretUpIcon size={16} /> : <ArrowUpRightIcon size={16} />}
        </button>
      )}
      {expanded ? (
        <p className="panel-note">
          Rejseplan journey planning and Buster live positions are not connected in this prototype.
        </p>
      ) : null}
    </section>
  );
}
