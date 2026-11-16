import { createFileRoute, useRouter } from "@tanstack/react-router"
import { useTransition } from "react"
import { ArrowClockwiseIcon } from "@phosphor-icons/react/dist/csr/ArrowClockwise"
import { ShieldCheckIcon } from "@phosphor-icons/react/dist/csr/ShieldCheck"
import { NavLayoutSwitch } from "@/components/nav-layout"
import { PageTitle } from "@/components/page-tools"
import { Button } from "@/components/ui/button"
import type { ConnectionState } from "@/lib/connection"
import { getConnectionHealth } from "@/lib/connection.functions"

export const Route = createFileRoute("/connection")({
  head: () => ({ meta: [{ title: "Connection · OpenHome" }] }),
  loader: () => getConnectionHealth(),
  headers: () => ({ "Cache-Control": "no-store" }),
  component: ConnectionPage,
  errorComponent: ConnectionError,
})

const outcomes: Record<
  ConnectionState,
  { label: string; title: string; detail: string }
> = {
  connected: {
    label: "Connected",
    title: "Your home is connected.",
    detail: "OpenHome accepted the API key and returned a healthy response.",
  },
  "missing-configuration": {
    label: "Setup needed",
    title: "Connect your home.",
    detail:
      "The dashboard needs a private server connection before it can reach OpenHome.",
  },
  "invalid-configuration": {
    label: "Check setup",
    title: "The connection needs a correction.",
    detail:
      "Ask the installer to check the server’s base URL and API key, then restart the dashboard.",
  },
  "rejected-credential": {
    label: "API key rejected",
    title: "OpenHome rejected the API key.",
    detail:
      "Ask the installer to update the dashboard’s API key to match OpenHome, then restart the dashboard.",
  },
  unreachable: {
    label: "Unreachable",
    title: "We couldn’t reach your home.",
    detail:
      "Check that OpenHome is running and the dashboard server has network access, then try again.",
  },
  unavailable: {
    label: "Unavailable",
    title: "OpenHome couldn’t confirm its health.",
    detail:
      "The server responded, but a healthy response wasn’t available. Check OpenHome and try again.",
  },
}

const checkTime = new Intl.DateTimeFormat("en-GB", {
  dateStyle: "medium",
  timeStyle: "medium",
  timeZone: "Europe/Copenhagen",
})

function ConnectionPage() {
  const health = Route.useLoaderData()
  const router = useRouter()
  const [isChecking, startTransition] = useTransition()
  const outcome = outcomes[health.state]

  function checkAgain() {
    startTransition(async () => {
      await router.invalidate({ sync: true })
    })
  }

  const label = isChecking ? "Checking…" : "Check again"

  return (
    <div
      className="group/link grid min-h-page-fill content-center gap-link-stack [@media(max-width:900px)]:min-h-0 [@media(max-width:900px)]:grid-cols-1 [@media(max-width:900px)]:items-start"
      data-state={health.state}
    >
      <div className="sr-only">
        <h1 id="page-heading" tabIndex={-1}>
          {outcome.title}
        </h1>
        <p>{outcome.detail}</p>
        <h2 id="status-heading">Connection status</h2>
      </div>
      <section
        aria-labelledby="status-heading"
        className="flex flex-wrap items-center gap-x-link-hero gap-y-6"
      >
        <span
          className="relative grid aspect-square w-signal shrink-0 place-items-center before:absolute before:inset-0 before:rounded-full before:bg-danger before:opacity-14 group-data-[state=connected]/link:before:bg-connected motion-safe:not-group-data-[state=connected]/link:animate-link-refuse motion-safe:group-data-[state=connected]/link:before:animate-link-signal"
          aria-hidden="true"
        >
          <span className="absolute inset-signal-core rounded-full bg-danger group-data-[state=connected]/link:bg-connected" />
        </span>
        <div role="status" aria-live="polite" aria-atomic="true">
          <p className="text-link-status leading-status font-medium tracking-status">
            {isChecking ? "Checking connection…" : outcome.label}
          </p>
          <p className="mt-3 text-control text-muted-ink tabular-nums">
            <span className="sr-only">Last checked </span>
            <time dateTime={health.checkedAt}>
              {checkTime.format(new Date(health.checkedAt))}
            </time>
          </p>
        </div>
      </section>
      <div className="flex flex-wrap items-center gap-x-8 gap-y-4">
        <Button
          size="key-hero"
          variant="outline"
          onClick={checkAgain}
          disabled={isChecking}
          data-spinning={isChecking}
          title={label}
        >
          <ArrowClockwiseIcon size={36} aria-hidden="true" />
          <span className="sr-only">{label}</span>
        </Button>
        <p className="flex items-center gap-2 text-control text-muted-ink">
          <ShieldCheckIcon
            size={20}
            className="shrink-0 text-ink"
            aria-hidden="true"
          />
          <span aria-hidden="true">Key stays on the server</span>
          <span className="sr-only">
            The API key stays on the dashboard server. It is never saved on this
            iPad.
          </span>
        </p>
      </div>
      <section aria-labelledby="layout-heading">
        <h2 id="layout-heading" className="sr-only">
          Navigation layout
        </h2>
        <NavLayoutSwitch />
      </section>
      {health.state === "missing-configuration" ? <SetupInstructions /> : null}
    </div>
  )
}

const setupCode =
  "rounded-code bg-wash-strong px-2.5 py-1 font-mono text-control"

function SetupInstructions() {
  return (
    <section aria-labelledby="setup-heading">
      <div className="sr-only">
        <h3 id="setup-heading">For the installer</h3>
        <p>
          Set these private environment variables on the dashboard server, then
          restart it and check again.
        </p>
      </div>
      <dl className="grid gap-3">
        <div className="flex flex-wrap items-baseline gap-x-4 gap-y-1">
          <dt>
            <code className={setupCode}>OPENHOME_API_URL</code>
          </dt>
          <dd className="text-control text-muted-ink">Axum API base URL</dd>
        </div>
        <div className="flex flex-wrap items-baseline gap-x-4 gap-y-1">
          <dt>
            <code className={setupCode}>API_KEY</code>
          </dt>
          <dd className="text-control text-muted-ink">
            The key configured on Axum
          </dd>
        </div>
      </dl>
    </section>
  )
}

function ConnectionError() {
  const router = useRouter()

  return (
    <div className="grid min-h-page-fill content-center justify-items-start gap-6">
      <PageTitle>The dashboard couldn’t check the connection.</PageTitle>
      <p className="max-w-120 text-lg leading-normal text-muted-ink">
        Check your connection to the dashboard and try again.
      </p>
      <Button
        size="lg"
        onClick={() => {
          void router.invalidate()
        }}
      >
        Try again
      </Button>
    </div>
  )
}
