import { useEffect, useId, useRef, useState } from "react"
import { Link } from "@tanstack/react-router"
import { FilmSlateIcon } from "@phosphor-icons/react/dist/csr/FilmSlate"
import { TelevisionIcon } from "@phosphor-icons/react/dist/csr/Television"
import { TrashIcon } from "@phosphor-icons/react/dist/csr/Trash"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select"
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogCancel,
} from "@/components/ui/alert-dialog"
import { createRequestId } from "@/lib/request-id"
import {
  watchlistCommand,
  watchlistFailureMessage,
  type WatchlistCommand,
  type WatchlistEntry,
  type WatchlistMutation,
  type WatchlistState,
} from "@/lib/watchlist"

type Feedback = { state: "saved" | "failed"; text: string }

/** All household watchlists, including empty owners, with explicit-destination forms. */
export function Watchlists({
  data,
  change,
  refresh,
}: {
  data: WatchlistState
  change: (command: WatchlistCommand) => Promise<WatchlistMutation>
  refresh: () => Promise<void>
}): React.JSX.Element {
  const [busy, setBusy] = useState(false)
  const locked = useRef(false)
  const [feedback, setFeedback] = useState<Feedback>()

  const [removal, setRemoval] = useState<{
    entry: WatchlistEntry
    ownerName: string
  }>()

  async function reload(): Promise<void> {
    if (locked.current) return
    locked.current = true
    setBusy(true)
    setFeedback(undefined)

    try {
      await refresh()
    } catch {
      setFeedback({
        state: "failed",
        text: "Could not refresh. Try again when OpenHome is reachable.",
      })
    } finally {
      locked.current = false
      setBusy(false)
    }
  }

  async function save(command: WatchlistCommand): Promise<boolean> {
    if (locked.current) return false
    locked.current = true
    setBusy(true)
    setFeedback(undefined)

    try {
      let result: WatchlistMutation

      try {
        result = await change(command)
      } catch {
        result = { state: "failed", reason: "unreachable" }
      }

      if (result.state === "failed") {
        setFeedback({
          state: "failed",
          text: watchlistFailureMessage[result.reason],
        })

        return false
      }

      setRemoval(undefined)
      setFeedback({
        state: "saved",
        text:
          command.action === "add"
            ? "Plan saved to OpenHome."
            : "Plan removed from OpenHome.",
      })

      try {
        await refresh()
      } catch {
        setFeedback({
          state: "saved",
          text: "Change saved to OpenHome. Refresh to see the current list.",
        })
      }

      return true
    } finally {
      locked.current = false
      setBusy(false)
    }
  }

  return (
    <div className="watchlists">
      <div className="wishlist-toolbar">
        <p className="section-label">
          {data.state === "available"
            ? `${data.entries.length} planned · ${data.owners.length} household members`
            : "Shared household watchlists"}
        </p>
        <Button variant="outline" disabled={busy} onClick={() => void reload()}>
          {busy ? "Working…" : "Refresh watchlists"}
        </Button>
      </div>
      <p
        className="wishlist-feedback"
        role="status"
        data-failed={feedback?.state === "failed"}
      >
        {feedback?.text}
      </p>
      {data.state === "unavailable" ? (
        <Card>
          <CardContent>
            <div className="watchlist-unavailable">
              <h2>Watchlists unavailable</h2>
              <p>{watchlistFailureMessage[data.reason]}</p>
              <Link to="/connection">Open Connection</Link>
            </div>
          </CardContent>
        </Card>
      ) : data.owners.length === 0 ? (
        <Card>
          <CardContent>
            <div className="watchlist-unavailable">
              <FilmSlateIcon size={40} aria-hidden="true" />
              <h2>A place for everyone’s next story.</h2>
              <p>
                Add a household member in Wishlist to start their watchlist.
                Members are shared across both views.
              </p>
              <Link to="/wishlist">Manage household members</Link>
            </div>
          </CardContent>
        </Card>
      ) : (
        <div className="watchlist-household">
          {data.owners.map((owner) => {
            const entries = data.entries.filter(
              (entry) => entry.owner_id === owner.id
            )

            return (
              <Card
                key={owner.id}
                role="group"
                aria-label={`${owner.name}’s watchlist`}
              >
                <CardHeader>
                  <div className="watchlist-owner-heading">
                    <div>
                      <p className="section-label">
                        Personal plans / Shared with everyone
                      </p>
                      <CardTitle>
                        <h2>{owner.name}</h2>
                      </CardTitle>
                    </div>
                    <span
                      className="watchlist-total"
                      aria-label={`${entries.length} planned`}
                    >
                      {entries.length.toString().padStart(2, "0")}
                    </span>
                  </div>
                </CardHeader>
                <CardContent>
                  <AddPlan
                    ownerId={owner.id}
                    ownerName={owner.name}
                    disabled={busy}
                    save={save}
                  />
                  {entries.length === 0 ? (
                    <div className="watchlist-empty">
                      <FilmSlateIcon size={32} aria-hidden="true" />
                      <p>No plans yet.</p>
                      <span>
                        Add a movie or an entire series for another evening.
                      </span>
                    </div>
                  ) : (
                    <ul
                      className="watchlist-entries"
                      aria-label={`Plans for ${owner.name}`}
                    >
                      {entries.map((entry) => (
                        <li key={entry.id}>
                          <span className="watchlist-kind-icon">
                            {entry.kind === "movie" ? (
                              <FilmSlateIcon size={24} aria-hidden="true" />
                            ) : (
                              <TelevisionIcon size={24} aria-hidden="true" />
                            )}
                          </span>
                          <div className="watchlist-entry-text">
                            <h3>{entry.title}</h3>
                            <p>
                              {entry.kind === "movie"
                                ? "Movie"
                                : "Whole series"}
                              <span aria-hidden="true"> · </span>
                              <span>Planned</span>
                            </p>
                            <p>
                              Added{" "}
                              <time dateTime={entry.date_added}>
                                {entry.date_added}
                              </time>
                            </p>
                          </div>
                          <Button
                            variant="ghost"
                            aria-label={`Remove ${entry.title} from ${owner.name}’s watchlist`}
                            disabled={busy}
                            onClick={() => {
                              setFeedback(undefined)
                              setRemoval({ entry, ownerName: owner.name })
                            }}
                          >
                            <TrashIcon aria-hidden="true" />
                          </Button>
                        </li>
                      ))}
                    </ul>
                  )}
                </CardContent>
              </Card>
            )
          })}
        </div>
      )}
      <AlertDialog
        open={removal !== undefined}
        onOpenChange={(open) => {
          if (!open && !busy) setRemoval(undefined)
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Remove this plan?</AlertDialogTitle>
            <AlertDialogDescription>
              Remove “{removal?.entry.title}” from {removal?.ownerName}’s
              watchlist? This cannot be undone.
            </AlertDialogDescription>
          </AlertDialogHeader>
          {feedback?.state === "failed" ? (
            <p role="status" className="wishlist-feedback" data-failed="true">
              {feedback.text}
            </p>
          ) : null}
          <AlertDialogFooter>
            <AlertDialogCancel disabled={busy}>Keep plan</AlertDialogCancel>
            <Button
              variant="destructive"
              disabled={busy}
              onClick={() => {
                if (removal)
                  void save({
                    action: "remove",
                    owner_id: removal.entry.owner_id,
                    entry_id: removal.entry.id,
                  })
              }}
            >
              Remove permanently
            </Button>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  )
}

type PendingPlan = Extract<WatchlistCommand, { action: "add" }>

function readPending(ownerId: string): PendingPlan | undefined {
  try {
    const parsed = watchlistCommand.safeParse(
      JSON.parse(
        sessionStorage.getItem(`openhome.pending-watch.${ownerId}`) ?? "null"
      )
    )

    if (
      parsed.success &&
      parsed.data.action === "add" &&
      parsed.data.owner_id === ownerId
    )
      return parsed.data
  } catch {
    /* Private browsing may disable storage. */
  }

  return undefined
}

function storePending(ownerId: string, pending: PendingPlan | undefined): void {
  try {
    const key = `openhome.pending-watch.${ownerId}`

    if (pending) sessionStorage.setItem(key, JSON.stringify(pending))
    else sessionStorage.removeItem(key)
  } catch {
    /* The mounted form retains its in-memory retry intention. */
  }
}

function AddPlan({
  ownerId,
  ownerName,
  disabled,
  save,
}: {
  ownerId: string
  ownerName: string
  disabled: boolean
  save: (command: WatchlistCommand) => Promise<boolean>
}): React.JSX.Element {
  const id = useId()
  const [title, setTitle] = useState("")
  const [kind, setKind] = useState<WatchlistEntry["kind"]>("movie")
  const pending = useRef<PendingPlan | undefined>(undefined)
  // Read tab storage after hydration, keeping SSR and the first client render identical.
  useEffect(() => {
    const recovered = readPending(ownerId)
    pending.current = recovered

    if (recovered) {
      // eslint-disable-next-line react/set-state-in-effect -- Restore a browser-owned retry draft after hydration.
      setTitle(recovered.title)
      setKind(recovered.kind)
    }
  }, [ownerId])

  async function submit(): Promise<void> {
    // Axum uses Rust's Unicode White_Space trim; JavaScript trim has different identity.
    const normalized = title.replace(
      /^\p{White_Space}+|\p{White_Space}+$/gu,
      ""
    )

    if (
      !pending.current ||
      pending.current.title !== normalized ||
      pending.current.kind !== kind
    ) {
      pending.current = {
        action: "add",
        owner_id: ownerId,
        title: normalized,
        kind,
        request_id: createRequestId(),
      }
    }

    storePending(ownerId, pending.current)

    if (await save(pending.current)) {
      pending.current = undefined
      storePending(ownerId, undefined)
      setTitle("")
    }
  }

  return (
    <form
      className="watchlist-add"
      aria-label={`Add plan for ${ownerName}`}
      onSubmit={(event) => {
        event.preventDefault()

        if (!disabled) void submit()
      }}
    >
      <div>
        <label htmlFor={`${id}-title`}>Title for {ownerName}</label>
        <Input
          id={`${id}-title`}
          name="title"
          value={title}
          onChange={(event) => setTitle(event.target.value)}
          required
          maxLength={500}
          autoComplete="off"
          disabled={disabled}
          placeholder="What would you like to watch?"
        />
      </div>
      <div className="watchlist-add-actions">
        <div>
          <label htmlFor={`${id}-kind`}>Type for {ownerName}</label>
          <NativeSelect
            id={`${id}-kind`}
            name="kind"
            value={kind}
            disabled={disabled}
            onChange={(event) => {
              const value = event.target.value

              if (value === "movie" || value === "series") setKind(value)
            }}
          >
            <NativeSelectOption value="movie">Movie</NativeSelectOption>
            <NativeSelectOption value="series">Whole series</NativeSelectOption>
          </NativeSelect>
        </div>
        <Button type="submit" disabled={disabled || title === ""}>
          Add plan
        </Button>
        <Button
          type="button"
          variant="ghost"
          disabled={disabled || title === ""}
          onClick={() => {
            pending.current = undefined
            storePending(ownerId, undefined)
            setTitle("")
          }}
        >
          Discard draft
        </Button>
      </div>
    </form>
  )
}
