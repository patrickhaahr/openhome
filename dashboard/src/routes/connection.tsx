import { createFileRoute, useRouter } from "@tanstack/react-router"
import { useTransition } from "react"
import { ArrowClockwiseIcon } from "@phosphor-icons/react/dist/csr/ArrowClockwise"
import { HouseSimpleIcon } from "@phosphor-icons/react/dist/csr/HouseSimple"
import { ShieldCheckIcon } from "@phosphor-icons/react/dist/csr/ShieldCheck"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardFooter, CardHeader } from "@/components/ui/card"
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

const checkTime = new Intl.DateTimeFormat("da-DK", {
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

  return (
    <div className="connection-page">
      <header className="flex items-center gap-3">
        <HouseSimpleIcon
          size={28}
          weight="duotone"
          className="text-primary"
          aria-hidden="true"
        />
        <span className="text-lg font-semibold tracking-tight">OpenHome</span>
      </header>

      <div className="grid flex-1 content-center items-start gap-10 lg:grid-cols-2 lg:gap-16">
        <section aria-labelledby="page-heading" className="flex flex-col gap-6">
          <p className="text-sm font-medium tracking-widest text-muted-foreground uppercase">
            Dashboard connection
          </p>
          <h1
            id="page-heading"
            tabIndex={-1}
            className="text-4xl leading-tight font-semibold tracking-tight sm:text-5xl"
          >
            {outcome.title}
          </h1>
          <p className="max-w-md text-lg leading-relaxed text-muted-foreground">
            {outcome.detail}
          </p>
          <div className="flex items-start gap-3 text-sm leading-relaxed text-muted-foreground">
            <ShieldCheckIcon
              size={24}
              className="shrink-0 text-primary"
              aria-hidden="true"
            />
            <p>
              The API key stays on the dashboard server. It is never saved on
              this iPad.
            </p>
          </div>
        </section>

        <Card>
          <CardHeader>
            <h2 className="text-xl font-semibold">Connection status</h2>
          </CardHeader>
          <CardContent>
            <div className="flex flex-col gap-6">
              <div
                role="status"
                aria-live="polite"
                aria-atomic="true"
                className="flex flex-col gap-2"
              >
                <p className="text-2xl font-medium">
                  {isChecking ? "Checking connection…" : outcome.label}
                </p>
                <p className="text-sm text-muted-foreground">
                  Last checked{" "}
                  <time dateTime={health.checkedAt}>
                    {checkTime.format(new Date(health.checkedAt))}
                  </time>
                </p>
              </div>
              {health.state === "missing-configuration" ? (
                <SetupInstructions />
              ) : null}
            </div>
          </CardContent>
          <CardFooter>
            <Button
              size="lg"
              onClick={checkAgain}
              disabled={isChecking}
              className="w-full"
            >
              <ArrowClockwiseIcon aria-hidden="true" data-icon="inline-start" />
              {isChecking ? "Checking…" : "Check again"}
            </Button>
          </CardFooter>
        </Card>
      </div>

      <footer className="text-sm text-muted-foreground">
        Private household dashboard
      </footer>
    </div>
  )
}

function SetupInstructions() {
  return (
    <div className="flex flex-col gap-3 border-t pt-6 text-sm leading-relaxed">
      <h3 className="font-semibold">For the installer</h3>
      <p className="text-muted-foreground">
        Set these private environment variables on the dashboard server, then
        restart it and check again.
      </p>
      <dl className="flex flex-col gap-3">
        <div>
          <dt className="font-mono text-primary">OPENHOME_API_URL</dt>
          <dd className="text-muted-foreground">Your Axum API base URL</dd>
        </div>
        <div>
          <dt className="font-mono text-primary">API_KEY</dt>
          <dd className="text-muted-foreground">
            The API key configured on Axum
          </dd>
        </div>
      </dl>
    </div>
  )
}

function ConnectionError() {
  const router = useRouter()

  return (
    <div className="mx-auto flex max-w-lg flex-col justify-center gap-6 p-8">
      <h1 id="page-heading" tabIndex={-1} className="text-3xl font-semibold">
        The dashboard couldn’t check the connection.
      </h1>
      <p className="text-muted-foreground">
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
