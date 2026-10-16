import { createFileRoute, useRouter } from "@tanstack/react-router"
import { useTransition } from "react"
import { ArrowClockwiseIcon } from "@phosphor-icons/react/dist/csr/ArrowClockwise"
import { ShieldCheckIcon } from "@phosphor-icons/react/dist/csr/ShieldCheck"
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
    <div className="link-page" data-state={health.state}>
      <div className="sr-only">
        <h1 id="page-heading" tabIndex={-1}>
          {outcome.title}
        </h1>
        <p>{outcome.detail}</p>
        <h2 id="status-heading">Connection status</h2>
      </div>
      <section aria-labelledby="status-heading" className="link-hero">
        <span className="link-signal" aria-hidden="true">
          <span />
        </span>
        <div role="status" aria-live="polite" aria-atomic="true">
          <p className="link-label">
            {isChecking ? "Checking connection…" : outcome.label}
          </p>
          <p className="link-checked">
            <span className="sr-only">Last checked </span>
            <time dateTime={health.checkedAt}>
              {checkTime.format(new Date(health.checkedAt))}
            </time>
          </p>
        </div>
      </section>
      <div className="link-tools">
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
        <p className="link-key">
          <ShieldCheckIcon size={20} aria-hidden="true" />
          <span aria-hidden="true">Key stays on the server</span>
          <span className="sr-only">
            The API key stays on the dashboard server. It is never saved on this
            iPad.
          </span>
        </p>
      </div>
      {health.state === "missing-configuration" ? <SetupInstructions /> : null}
    </div>
  )
}

function SetupInstructions() {
  return (
    <section className="link-setup" aria-labelledby="setup-heading">
      <div className="sr-only">
        <h3 id="setup-heading">For the installer</h3>
        <p>
          Set these private environment variables on the dashboard server, then
          restart it and check again.
        </p>
      </div>
      <dl>
        <div>
          <dt>
            <code>OPENHOME_API_URL</code>
          </dt>
          <dd>Axum API base URL</dd>
        </div>
        <div>
          <dt>
            <code>API_KEY</code>
          </dt>
          <dd>The key configured on Axum</dd>
        </div>
      </dl>
    </section>
  )
}

function ConnectionError() {
  const router = useRouter()

  return (
    <div className="lost">
      <h1 id="page-heading" tabIndex={-1} className="page-title">
        The dashboard couldn’t check the connection.
      </h1>
      <p className="lost-text">
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
