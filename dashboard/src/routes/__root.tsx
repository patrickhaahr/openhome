import {
  HeadContent,
  Link,
  Scripts,
  createRootRoute,
} from "@tanstack/react-router"
import { DashboardShell } from "@/components/dashboard-shell"

import appCss from "../styles.css?url"

export const Route = createRootRoute({
  loader: () => ({ openedAt: new Date().toISOString() }),
  headers: () => ({ "Cache-Control": "no-store" }),
  component: DashboardShell,
  head: () => ({
    meta: [
      {
        charSet: "utf-8",
      },
      {
        name: "viewport",
        content: "width=device-width, initial-scale=1",
      },
      {
        title: "OpenHome",
      },
    ],
    links: [
      {
        rel: "stylesheet",
        href: appCss,
      },
    ],
  }),
  notFoundComponent: () => (
    <section className="focused-page">
      <h1 id="page-heading" tabIndex={-1}>
        Page not found
      </h1>
      <p>The requested page could not be found.</p>
      <Link to="/" className="action-link">
        Back to Home
      </Link>
    </section>
  ),
  shellComponent: RootDocument,
})

function RootDocument({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className="dark">
      <head>
        <HeadContent />
      </head>
      <body>
        {children}
        <Scripts />
      </body>
    </html>
  )
}
