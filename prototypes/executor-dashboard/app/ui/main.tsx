import { createAppClient, mutationReference, queryReference } from "apps/client";
import { useAppQuery } from "apps/react";
import { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { IconContext } from "@phosphor-icons/react/dist/lib/context";
import type { addWish, wishlist } from "../index";
import { WishlistItem, WishlistItems } from "../schemas";
import { FeaturePage } from "./feature-pages";
import { pages, readPage } from "./navigation";
import type { Page } from "./navigation";
import type { ControlAction, DemoControls } from "./controls";
import type { DemoMemberId } from "./household";
import { initialWatches } from "./watchlist";
import type { DemoWatch } from "./watchlist";
import { Clock, useCurrentTime } from "./clock";
import "./style.css";

const client = createAppClient();
const wishesAtom = client.queryAtom(queryReference<typeof wishlist>("wishlist"), {}, WishlistItems);

function App(): React.JSX.Element {
  const now = useCurrentTime();
  const [page, setPage] = useState(readPage);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [watches, setWatches] = useState<ReadonlyArray<DemoWatch>>(initialWatches);
  const [controls, setControls] = useState<DemoControls>({
    lights: false,
    tv: false,
    speakerInput: "optical",
    speakerVolume: 35,
  });
  const { data, error } = useAppQuery(wishesAtom);
  const content = useRef<HTMLElement>(null);
  const label = pages.find((item) => item.id === page)?.label ?? "Home";

  function navigate(next: Page): void {
    if (next === page) return;
    const url = new URL(location.href);
    url.searchParams.delete("v");
    url.searchParams.set("page", next);
    history.pushState(null, "", url);
    setPage(next);
  }

  useEffect(() => {
    function restore(): void {
      setPage(readPage());
    }
    window.addEventListener("popstate", restore);
    return () => window.removeEventListener("popstate", restore);
  }, []);

  useEffect(() => {
    document.title = `${label} · OpenHome prototype`;
    content.current?.scrollTo(0, 0);
    const heading = content.current?.querySelector("h1");
    heading?.setAttribute("tabindex", "-1");
    heading?.focus({ preventScroll: true });
  }, [page, label]);

  function saveWish(owner: DemoMemberId, title: string): void {
    if (busy) return;
    setBusy(true);
    setMessage("");
    void client
      .mutate(
        mutationReference<typeof addWish>("addWish"),
        { id: crypto.randomUUID(), owner, title },
        WishlistItem,
      )
      .then(() => setMessage("Saved."))
      .catch(() => setMessage("Could not confirm the save. Check the list before retrying."))
      .finally(() => setBusy(false));
  }

  function control(action: ControlAction): void {
    setControls((current) => {
      switch (action.kind) {
        case "lights":
          return { ...current, lights: action.enabled };
        case "tv-power":
          return { ...current, tv: !current.tv };
        case "speaker-input":
          return { ...current, speakerInput: action.input };
        case "speaker-volume":
          return {
            ...current,
            speakerVolume: Math.max(0, Math.min(100, current.speakerVolume + action.change * 5)),
          };
      }
    });
  }

  const props = {
    now,
    wishes: data ?? [],
    busy,
    message,
    addWish: saveWish,
    navigate,
    watches,
    updateWatches: setWatches,
    controls,
    control,
  };
  return (
    <div className="dashboard-shell">
      <a className="skip-link" href="#page-content">
        Skip to page content
      </a>
      <aside className="sidebar">
        <nav className="sidebar-nav" aria-label="Main navigation">
          {pages.map((item) => (
            <a
              key={item.id}
              href={`?page=${item.id}`}
              aria-label={item.label}
              aria-current={page === item.id ? "page" : undefined}
              onClick={(event) => {
                if (
                  event.button !== 0 ||
                  event.ctrlKey ||
                  event.metaKey ||
                  event.shiftKey ||
                  event.altKey
                )
                  return;
                event.preventDefault();
                navigate(item.id);
              }}
            >
              <span className="nav-icon" aria-hidden="true">
                <item.icon />
              </span>
              <span className="sidebar-label">{item.shortLabel}</span>
            </a>
          ))}
        </nav>
        <div className="sidebar-bottom">
          <Clock now={now} className="sidebar-clock" />
        </div>
      </aside>
      <div className="dashboard-workspace">
        {error !== undefined ? (
          <div className="connection-error" role="alert">
            Executor tool connection failed: {error}
          </div>
        ) : null}
        <main
          ref={content}
          id="page-content"
          className={`page-content page-${page}`}
          aria-label={`${label} page`}
        >
          <FeaturePage key={page} page={page} {...props} />
          <footer className="page-footer">
            <span>Demo</span>
            <span>Executor v2</span>
          </footer>
        </main>
      </div>
    </div>
  );
}

const root = document.getElementById("root");
if (root === null) throw new Error("Prototype root element is missing.");
createRoot(root).render(
  <IconContext.Provider
    value={{ weight: "regular", size: 20, "aria-hidden": true, focusable: false }}
  >
    <App />
  </IconContext.Provider>,
);
