import { useState } from "react";
import { demoMembers } from "./household";

type Status = "planned" | "watching" | "completed";
/** Fictional media tracked in memory for the navigation prototype only. */
export type DemoWatch = {
  readonly id: string;
  readonly owner: string;
  readonly title: string;
  readonly kind: "movie" | "series";
  readonly status: Status;
  readonly added: string;
  readonly completed: string | null;
};
/** Isolated starter data, unrelated to any private viewing history. */
export const initialWatches: ReadonlyArray<DemoWatch> = [
  {
    id: "demo-film",
    owner: "Alex",
    title: "The Long Way Home",
    kind: "movie",
    status: "planned",
    added: "11/10/2026",
    completed: null,
  },
  {
    id: "demo-series",
    owner: "Alex",
    title: "City Days",
    kind: "series",
    status: "watching",
    added: "09/10/2026",
    completed: null,
  },
  {
    id: "demo-riley",
    owner: "Riley",
    title: "A Sunday Elsewhere",
    kind: "movie",
    status: "completed",
    added: "01/10/2026",
    completed: "10/10/2026",
  },
];
type Props = {
  readonly entries: ReadonlyArray<DemoWatch>;
  readonly update: (entries: ReadonlyArray<DemoWatch>) => void;
};

/** Demonstrates per-user lists and state transitions, without a production migration. */
export function WatchlistPage({ entries, update }: Props): React.JSX.Element {
  return (
    <div className="watchlist-page">
      <h1 className="sr-only">Watchlist</h1>
      <div className="household-lists">
        {demoMembers.map((member) => (
          <WatchlistGroup key={member.id} member={member} entries={entries} update={update} />
        ))}
      </div>
      <p className="panel-note">Demo · resets on reload</p>
    </div>
  );
}

function WatchlistGroup({
  member,
  entries,
  update,
}: Props & { readonly member: (typeof demoMembers)[number] }): React.JSX.Element {
  const owner = member.id;
  const [filter, setFilter] = useState<Status | "all">("all");
  const [title, setTitle] = useState("");
  const [kind, setKind] = useState<"movie" | "series">("movie");
  const rows = entries.filter(
    (entry) => entry.owner === owner && (filter === "all" || entry.status === filter),
  );
  function changeStatus(id: string, status: Status): void {
    update(
      entries.map((entry) =>
        entry.id === id
          ? {
              ...entry,
              status,
              completed: status === "completed" ? new Date().toLocaleDateString("en-GB") : null,
            }
          : entry,
      ),
    );
  }
  return (
    <section className="household-watchlist" aria-label={`${member.label} watchlist`}>
      <div className="member-heading">
        <span className="member-icon" aria-hidden="true">
          {member.icon}
        </span>
        <h2>{member.label}</h2>
        <span className="count-badge">{rows.length}</span>
      </div>
      <div className="segmented watch-filters" aria-label={`Watchlist filter for ${member.label}`}>
        {(["all", "planned", "watching", "completed"] as const).map((status) => (
          <button key={status} aria-pressed={filter === status} onClick={() => setFilter(status)}>
            {status === "all" ? "All" : status[0]?.toUpperCase() + status.slice(1)}
          </button>
        ))}
      </div>
      <div className="watch-grid">
        {rows.map((entry) => (
          <article className="card watch-entry" key={entry.id}>
            <div className={`demo-poster poster-${entry.kind}`} aria-hidden="true">
              <span>{entry.kind === "series" ? "SERIES" : "FILM"}</span>
              <strong>{entry.title}</strong>
              <i>Fictional cover</i>
            </div>
            <div className="watch-info">
              <span className="eyebrow">{entry.kind}</span>
              <h2>{entry.title}</h2>
              <label>
                <span className="sr-only">Status for {entry.title}</span>
                <select
                  aria-label={`Status for ${entry.title}`}
                  value={entry.status}
                  onChange={(event) => {
                    const value = event.target.value;
                    if (value === "planned" || value === "watching" || value === "completed")
                      changeStatus(entry.id, value);
                  }}
                >
                  <option value="planned">Planned</option>
                  <option value="watching">Watching</option>
                  <option value="completed">Completed</option>
                </select>
              </label>
              <p className="panel-note">
                Added {entry.added}
                {entry.completed === null ? "" : ` · Completed ${entry.completed}`}
              </p>
            </div>
          </article>
        ))}
      </div>
      {rows.length === 0 ? (
        <p className="empty-list">
          Nothing here yet. Add a demo title below, or choose another filter.
        </p>
      ) : null}
      <form
        className="card watch-add"
        aria-label={`Add a title for ${member.label}`}
        onSubmit={(event) => {
          event.preventDefault();
          const clean = title.trim();
          if (clean.length === 0) return;
          update([
            ...entries,
            {
              id: crypto.randomUUID(),
              owner,
              title: clean,
              kind,
              status: "planned",
              added: new Date().toLocaleDateString("en-GB"),
              completed: null,
            },
          ]);
          setTitle("");
          setFilter("all");
        }}
      >
        <label className="sr-only" htmlFor={`watch-title-${owner}`}>
          Movie or series title for {member.label}
        </label>
        <input
          id={`watch-title-${owner}`}
          value={title}
          maxLength={160}
          onChange={(event) => setTitle(event.target.value)}
          placeholder="Add a movie or series…"
        />
        <select
          aria-label={`Media type for ${member.label}`}
          value={kind}
          onChange={(event) => {
            if (event.target.value === "movie" || event.target.value === "series")
              setKind(event.target.value);
          }}
        >
          <option value="movie">Movie</option>
          <option value="series">Series</option>
        </select>
        <button className="primary-button" disabled={title.trim().length === 0} type="submit">
          Add title
        </button>
      </form>
    </section>
  );
}
