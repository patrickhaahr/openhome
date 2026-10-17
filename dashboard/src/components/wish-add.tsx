import { useEffect, useId, useRef, useState } from "react"
import { CircleNotchIcon } from "@phosphor-icons/react/dist/csr/CircleNotch"
import { LinkSimpleIcon } from "@phosphor-icons/react/dist/csr/LinkSimple"
import { PlusIcon } from "@phosphor-icons/react/dist/csr/Plus"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { PersonDisc } from "@/components/list-parts"
import { WishArt } from "@/components/wish-sheet"
import {
  WishFields,
  detailsOf,
  emptyWish,
  type WishDraft,
} from "@/components/wish-fields"
import { trimText } from "@/lib/lists"
import { createRequestId } from "@/lib/request-id"
import {
  readPendingAddition,
  storePendingAddition,
  type PendingAddition,
} from "@/lib/wishlist-addition"
import type {
  HouseholdUser,
  Wishlist,
  WishlistCommand,
  WishlistDetails,
  WishlistMetadataLoader,
} from "@/lib/wishlist"

type Suggested = "title" | "image_url" | "price"

type Reading =
  | { state: "idle" }
  | { state: "reading"; url: string }
  | { state: "read"; url: string; message: string }

function readableUrl(value: string): string | undefined {
  try {
    const url = new URL(trimText(value))

    return url.protocol === "http:" || url.protocol === "https:"
      ? url.href
      : undefined
  } catch {
    return undefined
  }
}

function draftFromDetails(details: WishlistDetails): WishDraft {
  return {
    title: details.title,
    url: details.url ?? "",
    image_url: details.image_url ?? "",
    notes: details.notes ?? "",
    price: details.price ?? "",
    quantity: details.quantity ?? 1,
    priority:
      details.priority === "low" || details.priority === "high"
        ? details.priority
        : "normal",
  }
}

/** Field by field, since stored details may come back in another key order. */
function sameDetails(a: WishlistDetails, b: WishlistDetails): boolean {
  return (
    a.title === b.title &&
    (a.url ?? null) === (b.url ?? null) &&
    (a.image_url ?? null) === (b.image_url ?? null) &&
    (a.notes ?? null) === (b.notes ?? null) &&
    (a.price ?? null) === (b.price ?? null) &&
    (a.quantity ?? 1) === (b.quantity ?? 1) &&
    (a.priority ?? "normal") === (b.priority ?? "normal")
  )
}

/**
 * Paste a link and the product is read into editable fields; anything missing is typed. Who it
 * is for and which list come last, but the link is only read once a list is chosen, because
 * Axum checks the destination. Nothing is saved until "Add wish".
 */
export function WishAdd({
  owners,
  lists,
  person,
  list,
  busy,
  save,
  metadata,
  done,
}: {
  owners: readonly HouseholdUser[]
  lists: readonly Wishlist[]
  person: string | undefined
  list: string | undefined
  busy: boolean
  save: (command: WishlistCommand, saved: string) => Promise<boolean>
  metadata: WishlistMetadataLoader
  done: () => void
}): React.JSX.Element {
  const id = useId()

  const [initial] = useState(() => {
    const target =
      list !== undefined && lists.some((item) => item.id === list)
        ? list
        : undefined

    // Opened on a list whose last wish lost its response: resume that wish.
    const recovered =
      target === undefined ? undefined : readPendingAddition(target)

    return {
      target,
      recovered: recovered?.details === undefined ? undefined : recovered,
    }
  })

  const [url, setUrl] = useState(initial.recovered?.details?.url ?? "")

  const [draft, setDraft] = useState<WishDraft>(() =>
    initial.recovered?.details === undefined
      ? emptyWish
      : draftFromDetails(initial.recovered.details)
  )

  const [suggested, setSuggested] = useState<ReadonlySet<Suggested>>(new Set())
  const [reading, setReading] = useState<Reading>({ state: "idle" })
  const [ownerId, setOwnerId] = useState(person)
  const [listId, setListId] = useState(initial.target)
  const [newList, setNewList] = useState<string>()
  const [awaitedList, setAwaitedList] = useState<string>()
  const latest = useRef(0)
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined)
  const pending = useRef<PendingAddition | undefined>(initial.recovered)

  // A closed plate reads nothing more.
  useEffect(() => () => clearTimeout(timer.current), [])
  const ownerLists = lists.filter((item) => item.owner_id === ownerId)

  // A list created here appears after the re-read; choose it as soon as it does.
  const created =
    awaitedList === undefined
      ? undefined
      : ownerLists.find((item) => item.name === awaitedList)

  if (created !== undefined) {
    setAwaitedList(undefined)
    setListId(created.id)
  }

  async function read(link: string, owner: string, target: string) {
    const ticket = ++latest.current
    setReading({ state: "reading", url: link })

    let message =
      "Could not read this link. Fill in the details yourself; the link is kept."

    try {
      const result = await metadata({
        owner_id: owner,
        list_id: target,
        url: link,
      })

      if (ticket !== latest.current) return

      if (result.state === "preview" && result.data.status !== "unavailable") {
        const data = result.data
        const filled = new Set<Suggested>()

        setDraft((current) => {
          const next = { ...current }

          for (const field of ["title", "image_url", "price"] as const) {
            if (current[field] !== "" && !suggested.has(field)) continue

            next[field] = data[field] ?? ""
            filled.add(field)
          }

          return next
        })
        setSuggested(filled)
        message =
          data.status === "ready"
            ? "Read from the link. Check the details before adding."
            : "Partly read from the link. Fill in what is missing."
      }
    } catch {
      if (ticket !== latest.current) return
    }

    setReading({ state: "read", url: link, message })
  }

  function maybeRead(link: string, owner?: string, target?: string) {
    clearTimeout(timer.current)
    const readable = readableUrl(link)

    if (readable === undefined || owner === undefined || target === undefined)
      return

    timer.current = setTimeout(() => void read(readable, owner, target), 450)
  }

  function chooseList(target: string) {
    setListId(target)

    // A wish whose response was lost earlier resumes with its request ID.
    const recovered = readPendingAddition(target)

    if (recovered?.details !== undefined && trimText(draft.title) === "") {
      pending.current = recovered
      setUrl(recovered.details.url ?? "")
      setDraft(draftFromDetails(recovered.details))

      return
    }

    if (
      reading.state === "idle" ||
      (reading.state === "read" && reading.url !== readableUrl(url))
    )
      maybeRead(url, ownerId, target)
  }

  async function submit(owner: string, target: string) {
    const details = detailsOf({ ...draft, url })

    // The same wish to the same list keeps its request ID, even after a reload.
    const stored = pending.current ?? readPendingAddition(target)

    pending.current =
      stored !== undefined &&
      stored.listId === target &&
      (stored.details === undefined
        ? trimText(stored.title) === details.title
        : sameDetails(stored.details, details))
        ? { ...stored, title: details.title, details }
        : {
            ownerId: owner,
            listId: target,
            title: details.title,
            details,
            requestId: createRequestId(),
          }

    storePendingAddition(target, pending.current)

    const saved = await save(
      {
        kind: "add-item",
        owner_id: owner,
        list_id: target,
        ...details,
        request_id: pending.current.requestId,
      },
      `Added “${details.title}”.`
    )

    if (saved) {
      pending.current = undefined
      storePendingAddition(target, undefined)
      done()
    }
  }

  const ready =
    ownerId !== undefined &&
    listId !== undefined &&
    trimText(draft.title) !== ""

  return (
    <form
      className="sheet-body"
      aria-label="Add a wish"
      onSubmit={(event) => {
        event.preventDefault()

        if (ready && !busy) void submit(ownerId, listId)
      }}
    >
      <label className="field" htmlFor={`${id}-link`}>
        <span>Paste a link</span>
        <span className="link-input">
          <LinkSimpleIcon size={22} aria-hidden="true" />
          <Input
            id={`${id}-link`}
            name="url"
            type="url"
            inputMode="url"
            value={url}
            autoFocus
            onChange={(event) => {
              const link = event.target.value
              setUrl(link)

              if (readableUrl(link) !== readableUrl(url)) {
                // A new link replaces what the last one suggested, never what was typed.
                setDraft((current) => {
                  const next = { ...current }

                  for (const field of suggested) next[field] = ""

                  return next
                })
                setSuggested(new Set())
                setReading({ state: "idle" })
                maybeRead(link, ownerId, listId)
              }
            }}
            maxLength={2048}
            disabled={busy}
            placeholder="https://…"
          />
          {reading.state === "reading" ? (
            <CircleNotchIcon size={22} className="spin" aria-hidden="true" />
          ) : null}
        </span>
      </label>
      <p className="link-status" role="status">
        {reading.state === "reading"
          ? "Reading the link…"
          : reading.state === "read"
            ? reading.message
            : readableUrl(url) !== undefined && listId === undefined
              ? "Choose who it is for, and the link is read."
              : ""}
      </p>
      <div className="wish-add-preview">
        {/* An unsaved picture is never fetched from the shop; it shows once the wish is saved. */}
        <WishArt title={draft.title || "New wish"} image={null} />
        <WishFields
          draft={draft}
          edit={(change) => {
            setDraft((current) => ({ ...current, ...change }))
            setSuggested((current) => {
              const next = new Set(current)

              for (const field of ["title", "image_url", "price"] as const)
                if (field in change) next.delete(field)

              return next
            })
          }}
          disabled={busy}
          withLink={false}
        />
      </div>
      <fieldset className="field destination">
        <legend>For</legend>
        <div className="view-tabs person-keys">
          {owners.map((owner) => (
            <button
              key={owner.id}
              type="button"
              className="view-tab person-key"
              aria-pressed={ownerId === owner.id}
              disabled={busy}
              onClick={() => {
                if (owner.id === ownerId) return
                setOwnerId(owner.id)
                setListId(undefined)
                setNewList(undefined)
              }}
            >
              <PersonDisc name={owner.name} />
              {owner.name}
            </button>
          ))}
        </div>
      </fieldset>
      {ownerId === undefined ? null : (
        <fieldset className="field destination">
          <legend>List</legend>
          <div className="list-keys">
            {ownerLists.map((item) => (
              <button
                key={item.id}
                type="button"
                className="list-key"
                aria-pressed={listId === item.id}
                disabled={busy}
                onClick={() => chooseList(item.id)}
              >
                {item.name}
              </button>
            ))}
            {newList === undefined ? (
              <button
                type="button"
                className="list-key list-key-new"
                disabled={busy}
                onClick={() => setNewList("")}
              >
                <PlusIcon size={16} weight="bold" aria-hidden="true" />
                New list
              </button>
            ) : (
              <span className="new-list">
                <label className="sr-only" htmlFor={`${id}-list`}>
                  New list name
                </label>
                <Input
                  id={`${id}-list`}
                  value={newList}
                  maxLength={200}
                  autoFocus
                  disabled={busy}
                  placeholder="List name"
                  onChange={(event) => setNewList(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") event.preventDefault()
                  }}
                />
                <Button
                  type="button"
                  variant="outline"
                  disabled={busy || trimText(newList) === ""}
                  onClick={() => {
                    const name = trimText(newList)

                    void save(
                      { kind: "create-list", owner_id: ownerId, name },
                      `Created the list ${name}.`
                    ).then((saved) => {
                      if (saved) {
                        setNewList(undefined)
                        setAwaitedList(name)
                      }
                    })
                  }}
                >
                  Create
                </Button>
              </span>
            )}
          </div>
        </fieldset>
      )}
      <div className="sheet-actions sheet-submit">
        <Button type="submit" size="key-xl" disabled={!ready || busy}>
          <PlusIcon size={20} weight="bold" aria-hidden="true" />
          Add wish
        </Button>
      </div>
    </form>
  )
}
