import {
  HeadContent,
  Link,
  Scripts,
  createRootRoute,
} from "@tanstack/react-router"
import { HouseIcon } from "@phosphor-icons/react/dist/csr/House"
import { DashboardShell } from "@/components/dashboard-shell"
import { ThemeScript } from "@/components/daylight"
import { daylight } from "@/lib/daylight"
import { readIdleSettings } from "@/lib/idle.functions"
import { readNavPrefs } from "@/lib/nav-layout.functions"

import appCss from "../styles.css?url"

export const Route = createRootRoute({
  loader: () => ({
    openedAt: new Date().toISOString(),
    nav: readNavPrefs(),
    idle: readIdleSettings(),
  }),
  headers: () => ({ "Cache-Control": "no-store" }),
  component: DashboardShell,
  head: () => ({
    meta: [
      {
        charSet: "utf-8",
      },
      {
        name: "viewport",
        content: "width=device-width, initial-scale=1, viewport-fit=cover",
      },
      { name: "color-scheme", content: "light dark" },
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
    <section className="lost">
      <p className="lost-code num" aria-hidden="true">
        404
      </p>
      <h1 id="page-heading" tabIndex={-1} className="lost-title">
        Page not found
      </h1>
      <p className="sr-only">There is no room by that name in this home.</p>
      <Link to="/" className="dir-go lost-home" title="Back to Home">
        <HouseIcon size={24} aria-hidden="true" />
        <span className="sr-only">Back to Home</span>
      </Link>
    </section>
  ),
  shellComponent: RootDocument,
})

function RootDocument({ children }: { children: React.ReactNode }) {
  const { openedAt } = Route.useLoaderData()

  return (
    <html lang="en" suppressHydrationWarning>
      <head>
        <ThemeScript phase={daylight(new Date(openedAt)).phase} />
        <HeadContent />
      </head>
      <body>
        {children}
        <Scripts />
      </body>
    </html>
  )
}
