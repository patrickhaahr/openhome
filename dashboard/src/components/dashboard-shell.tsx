import { Link, Outlet, useRouter } from "@tanstack/react-router"
import { useEffect } from "react"
import { HouseSimpleIcon } from "@phosphor-icons/react/dist/csr/HouseSimple"
import { PlugsConnectedIcon } from "@phosphor-icons/react/dist/csr/PlugsConnected"
import { features, type Feature } from "@/lib/features"

/** Persistent navigation with one content landmark and focus after rendered navigation. */
export function DashboardShell(): React.JSX.Element {
  const router = useRouter()

  useEffect(
    () =>
      router.subscribe("onRendered", ({ pathChanged }) => {
        if (pathChanged)
          document
            .getElementById("page-heading")
            ?.focus({ preventScroll: true })
      }),
    [router]
  )

  return (
    <div className="dashboard-shell">
      <a className="skip-link" href="#page-content">
        Skip to page content
      </a>
      <aside className="sidebar">
        <div className="sidebar-brand" aria-label="OpenHome">
          OH<span aria-hidden="true">.</span>
        </div>
        <nav className="sidebar-nav" aria-label="Main navigation">
          <Link to="/" className="nav-link" activeOptions={{ exact: true }}>
            <HouseSimpleIcon size={24} aria-hidden="true" />
            <span>Home</span>
          </Link>
          {features.map((feature) => {
            const content = (
              <>
                <feature.icon size={24} aria-hidden="true" />
                <span>{feature.label}</span>
              </>
            )

            const to = implementedRoute(feature.id)

            // Implemented features use their own typed routes; the rest share the placeholder.
            return to === undefined ? (
              <Link
                key={feature.id}
                to="/$feature"
                params={{ feature: feature.id }}
                className="nav-link"
              >
                {content}
              </Link>
            ) : (
              <Link key={feature.id} to={to} className="nav-link">
                {content}
              </Link>
            )
          })}
          <Link to="/connection" className="nav-link connection-link">
            <PlugsConnectedIcon size={24} aria-hidden="true" />
            <span>Connection</span>
          </Link>
        </nav>
      </aside>
      <div className="dashboard-workspace">
        <main id="page-content" tabIndex={-1} className="page-content">
          <Outlet />
        </main>
        <footer className="page-footer">
          <span>OpenHome</span>
          <span>Private household display</span>
        </footer>
      </div>
    </div>
  )
}

function implementedRoute(
  id: Feature["id"]
): "/controls" | "/weather" | "/travel" | "/wishlist" | undefined {
  switch (id) {
    case "controls":
      return "/controls"
    case "weather":
      return "/weather"
    case "travel":
      return "/travel"
    case "wishlist":
      return "/wishlist"
    case "agent":
    case "photos":
    case "server":
    case "watchlist":
    case "workout":
      return undefined
  }
}
