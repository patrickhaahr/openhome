import { useEffect, useId, useRef, useState } from "react"
import { createRequestId } from "@/lib/request-id"
import { HeartIcon } from "@phosphor-icons/react/dist/csr/Heart"
import { PencilSimpleIcon } from "@phosphor-icons/react/dist/csr/PencilSimple"
import { TrashIcon } from "@phosphor-icons/react/dist/csr/Trash"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { ItemForm } from "@/components/wishlist-item-form"
import { ItemDetails } from "@/components/wishlist-item-details"
import {
  readPendingAddition,
  storePendingAddition,
  type PendingAddition,
} from "@/lib/wishlist-addition"
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog"
import {
  wishlistFailureMessage,
  type WishlistCommand,
  type WishlistMutation,
  type WishlistState,
  type WishlistMetadataLoader,
} from "@/lib/wishlist"

type TextFormProps = {
  label: string
  initialValue?: string
  submitLabel: string
  maxLength: number
  disabled: boolean
  save: (value: string) => Promise<boolean>
  cancel?: () => void
}

function trimWishlistText(value: string): string {
  // Match Axum's Rust str::trim(): Unicode White_Space, unlike JS trim().
  return value.replace(/^\p{White_Space}+|\p{White_Space}+$/gu, "")
}

function TextForm({
  label,
  initialValue = "",
  submitLabel,
  maxLength,
  disabled,
  save,
  cancel,
}: TextFormProps): React.JSX.Element {
  const id = useId()

  const [draft, setDraft] = useState({
    source: initialValue,
    value: initialValue,
  })

  // Refresh pristine fields before rendering, while keeping unsaved edits.
  if (draft.source !== initialValue)
    setDraft({
      source: initialValue,
      value: draft.value === draft.source ? initialValue : draft.value,
    })

  const value = draft.value

  return (
    <form
      className="wishlist-text-form"
      onSubmit={(event) => {
        event.preventDefault()

        void save(value).then((saved) => {
          if (saved)
            setDraft((current) => ({
              source: current.source,
              value: initialValue === "" ? "" : trimWishlistText(value),
            }))
        })
      }}
    >
      <label htmlFor={id}>{label}</label>
      <div className="wishlist-form-row">
        <Input
          id={id}
          value={value}
          onChange={(event) =>
            setDraft({ source: draft.source, value: event.target.value })
          }
          required
          maxLength={maxLength}
          disabled={disabled}
          autoComplete="off"
        />
        <Button
          type="submit"
          disabled={disabled || trimWishlistText(value) === ""}
        >
          {submitLabel}
        </Button>
        {cancel ? (
          <Button
            type="button"
            variant="ghost"
            disabled={disabled}
            onClick={cancel}
          >
            Cancel
          </Button>
        ) : null}
      </div>
    </form>
  )
}

type Removal = { command: WishlistCommand; title: string; description: string }

/** Shared household browser with explicit destinations and persistence-confirmed feedback. */
export function Wishlists({
  data,
  change,
  refresh,
  metadata,
}: {
  readonly data: WishlistState
  readonly change: (command: WishlistCommand) => Promise<WishlistMutation>
  readonly refresh: () => Promise<void>
  readonly metadata: WishlistMetadataLoader
}): React.JSX.Element {
  const [selectedId, setSelectedId] = useState<string>()
  const [editing, setEditing] = useState<string>()
  const [removal, setRemoval] = useState<Removal>()
  const [busy, setBusy] = useState(false)
  const locked = useRef(false)
  const [feedback, setFeedback] = useState<{ failed: boolean; text: string }>()

  async function reload(): Promise<void> {
    if (locked.current) return
    locked.current = true
    setBusy(true)

    try {
      await refresh()
      setFeedback({ failed: false, text: "Wishlists refreshed." })
    } catch {
      setFeedback({
        failed: true,
        text: "Could not refresh. Try again when OpenHome returns.",
      })
    } finally {
      locked.current = false
      setBusy(false)
    }
  }

  async function save(command: WishlistCommand): Promise<boolean> {
    if (locked.current) return false
    locked.current = true
    setBusy(true)
    setFeedback(undefined)
    let saved = false

    try {
      const result = await change(command)

      if (result.state === "failed") {
        setFeedback({
          failed: true,
          text: wishlistFailureMessage[result.reason],
        })

        return false
      }

      saved = true
      setEditing(undefined)
      setRemoval(undefined)
      await refresh()
      setFeedback({ failed: false, text: "Saved to OpenHome." })

      return true
    } catch {
      setFeedback({
        failed: true,
        text: saved
          ? "Saved to OpenHome, but the view could not refresh. Refresh to see the change."
          : wishlistFailureMessage.unreachable,
      })

      return saved
    } finally {
      locked.current = false
      setBusy(false)
    }
  }

  const selected =
    data.state === "available"
      ? (data.lists.find((list) => list.id === selectedId) ?? data.lists[0])
      : undefined

  const selectedOwner =
    data.state === "available"
      ? data.owners.find((owner) => owner.id === selected?.owner_id)
      : undefined

  const selectedItems =
    data.state === "available"
      ? data.items.filter((item) => item.list_id === selected?.id)
      : []

  return (
    <div className="wishlists" aria-busy={busy}>
      <div className="wishlist-toolbar">
        <p className="page-eyebrow">Shared with your household</p>
        <Button
          variant="outline"
          disabled={busy}
          onClick={() => {
            void reload()
          }}
        >
          Refresh lists
        </Button>
      </div>
      <p
        className="wishlist-feedback"
        role="status"
        data-failed={feedback?.failed}
      >
        {feedback?.text ?? (busy ? "Saving…" : "")}
      </p>
      {data.state === "unavailable" ? (
        <Card>
          <CardHeader>
            <h2>Wishlists unavailable</h2>
          </CardHeader>
          <CardContent>
            <p>
              {data.reason === "unreachable"
                ? "OpenHome is unreachable. Refresh lists when the connection returns."
                : wishlistFailureMessage[data.reason]}
            </p>
          </CardContent>
        </Card>
      ) : (
        <>
          <div className="wishlist-layout">
            <aside
              className="wishlist-household"
              aria-label="Household wishlists"
            >
              {data.owners.length === 0 ? (
                <p className="wishlist-empty">
                  Add a household member below, then create their first list.
                </p>
              ) : null}
              {data.owners.map((owner) => {
                const lists = data.lists.filter(
                  (list) => list.owner_id === owner.id
                )

                return (
                  <section
                    className="wishlist-owner"
                    key={owner.id}
                    aria-label={`${owner.name}’s lists`}
                  >
                    <h2>{owner.name}</h2>
                    {lists.length === 0 ? (
                      <p className="wishlist-empty">No lists yet.</p>
                    ) : null}
                    <div className="wishlist-list-buttons">
                      {lists.map((list) => (
                        <Button
                          key={list.id}
                          variant="ghost"
                          aria-pressed={selected?.id === list.id}
                          disabled={busy}
                          onClick={() => {
                            setSelectedId(list.id)
                            setEditing(undefined)
                          }}
                        >
                          <span>{list.name}</span>
                          <span className="wishlist-count">
                            {wishCount(
                              data.items.filter(
                                (item) => item.list_id === list.id
                              ).length
                            )}
                          </span>
                        </Button>
                      ))}
                    </div>
                    <details className="wishlist-create">
                      <summary>New list for {owner.name}</summary>
                      <TextForm
                        label={`List name for ${owner.name}`}
                        submitLabel="Create list"
                        maxLength={200}
                        disabled={busy}
                        save={(name) =>
                          save({
                            kind: "create-list",
                            owner_id: owner.id,
                            name,
                          })
                        }
                      />
                    </details>
                  </section>
                )
              })}
            </aside>
            <Card>
              {selected && selectedOwner ? (
                <>
                  <CardHeader>
                    <div className="wishlist-detail-header">
                      <div>
                        <p className="page-eyebrow">
                          {selectedOwner.name}’s wishlist
                        </p>
                        <h2>{selected.name}</h2>
                      </div>
                      <HeartIcon size={32} aria-hidden="true" />
                    </div>
                    <div className="wishlist-actions">
                      <Button
                        variant="outline"
                        disabled={busy}
                        onClick={() => setEditing(`list-${selected.id}`)}
                      >
                        <PencilSimpleIcon aria-hidden="true" />
                        Rename list
                      </Button>
                      <Button
                        variant="ghost"
                        disabled={busy}
                        onClick={() =>
                          setRemoval({
                            command: {
                              kind: "remove-list",
                              owner_id: selected.owner_id,
                              list_id: selected.id,
                            },
                            title: `Remove ${selected.name}?`,
                            description: `This removes ${selectedOwner.name}’s list and all ${wishCount(selectedItems.length)} in it. This cannot be undone.`,
                          })
                        }
                      >
                        <TrashIcon aria-hidden="true" />
                        Remove list
                      </Button>
                    </div>
                  </CardHeader>
                  <CardContent>
                    {editing === `list-${selected.id}` ? (
                      <TextForm
                        key={editing}
                        label="List name"
                        initialValue={selected.name}
                        submitLabel="Save name"
                        maxLength={200}
                        disabled={busy}
                        cancel={() => setEditing(undefined)}
                        save={(name) =>
                          save({
                            kind: "rename-list",
                            owner_id: selected.owner_id,
                            list_id: selected.id,
                            name,
                          })
                        }
                      />
                    ) : null}
                    <AddWish
                      key={selected.id}
                      ownerId={selected.owner_id}
                      listId={selected.id}
                      disabled={busy}
                      save={save}
                      metadata={metadata}
                    />
                    {selectedItems.length === 0 ? (
                      <div className="wishlist-empty-detail">
                        <HeartIcon size={40} aria-hidden="true" />
                        <h3>A place for ideas.</h3>
                        <p>
                          Add something you’d like. A simple title is enough.
                        </p>
                      </div>
                    ) : null}
                    <ul
                      className="wishlist-items"
                      aria-label={`Wishes in ${selected.name}`}
                    >
                      {selectedItems.map((item, index) => (
                        <li key={item.id}>
                          {editing === `item-${item.id}` ? (
                            <ItemForm
                              label="Wish title"
                              initialValue={item}
                              submitLabel="Save wish"
                              disabled={busy}
                              cancel={() => setEditing(undefined)}
                              save={(details) =>
                                save({
                                  kind: "edit-item",
                                  owner_id: selected.owner_id,
                                  list_id: selected.id,
                                  item_id: item.id,
                                  title:
                                    details.title === item.title
                                      ? undefined
                                      : details.title,
                                  url:
                                    details.url === item.url
                                      ? undefined
                                      : details.url,
                                  image_url:
                                    details.image_url === item.image_url
                                      ? undefined
                                      : details.image_url,
                                  notes:
                                    details.notes === item.notes
                                      ? undefined
                                      : details.notes,
                                  price:
                                    details.price === item.price
                                      ? undefined
                                      : details.price,
                                  quantity:
                                    details.quantity === item.quantity
                                      ? undefined
                                      : details.quantity,
                                  priority:
                                    details.priority === item.priority
                                      ? undefined
                                      : details.priority,
                                })
                              }
                            />
                          ) : (
                            <>
                              <span
                                className="wishlist-item-number"
                                aria-hidden="true"
                              >
                                {String(index + 1).padStart(2, "0")}
                              </span>
                              <ItemDetails item={item} />
                              <div className="wishlist-item-actions">
                                <Button
                                  variant="ghost"
                                  disabled={busy}
                                  aria-label={`Edit ${item.title}`}
                                  onClick={() => setEditing(`item-${item.id}`)}
                                >
                                  <PencilSimpleIcon aria-hidden="true" />
                                </Button>
                                <Button
                                  variant="ghost"
                                  disabled={busy}
                                  aria-label={`Remove ${item.title}`}
                                  onClick={() =>
                                    setRemoval({
                                      command: {
                                        kind: "remove-item",
                                        owner_id: selected.owner_id,
                                        list_id: selected.id,
                                        item_id: item.id,
                                      },
                                      title: "Remove this wish?",
                                      description: `Remove “${item.title}” from ${selectedOwner.name}’s ${selected.name} list? This cannot be undone.`,
                                    })
                                  }
                                >
                                  <TrashIcon aria-hidden="true" />
                                </Button>
                              </div>
                            </>
                          )}
                        </li>
                      ))}
                    </ul>
                  </CardContent>
                </>
              ) : (
                <CardContent>
                  <div className="wishlist-empty-detail">
                    <HeartIcon size={40} aria-hidden="true" />
                    <h2>Your ideas belong here.</h2>
                    <p>
                      Create a named list beside a household member to get
                      started.
                    </p>
                  </div>
                </CardContent>
              )}
            </Card>
          </div>
          <details className="wishlist-settings">
            <summary>Manage household members</summary>
            <p className="wishlist-empty">
              Names are shared across household lists. Everyone can see and edit
              these lists.
            </p>
            <div className="wishlist-member-settings">
              {data.owners.map((owner) => (
                <section key={owner.id} aria-label={`Manage ${owner.name}`}>
                  <TextForm
                    label={`Name for ${owner.name}`}
                    initialValue={owner.name}
                    submitLabel="Rename member"
                    maxLength={200}
                    disabled={busy}
                    save={(name) =>
                      save({ kind: "rename-owner", owner_id: owner.id, name })
                    }
                  />
                  <Button
                    variant="ghost"
                    disabled={busy}
                    onClick={() =>
                      setRemoval({
                        command: { kind: "remove-owner", owner_id: owner.id },
                        title: `Remove ${owner.name}?`,
                        description:
                          "This removes the household member and all their wishlists, wishes and watchlist entries. This cannot be undone.",
                      })
                    }
                  >
                    Remove {owner.name}
                  </Button>
                </section>
              ))}
              <TextForm
                label="New household member"
                submitLabel="Add member"
                maxLength={200}
                disabled={busy}
                save={(name) => save({ kind: "create-owner", name })}
              />
            </div>
          </details>
        </>
      )}
      <AlertDialog
        open={removal !== undefined}
        onOpenChange={(open) => {
          if (!open && !busy) setRemoval(undefined)
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{removal?.title}</AlertDialogTitle>
            <AlertDialogDescription>
              {removal?.description}
            </AlertDialogDescription>
          </AlertDialogHeader>
          {feedback?.failed === true ? (
            <p role="status" className="wishlist-feedback" data-failed="true">
              {feedback.text}
            </p>
          ) : null}
          <AlertDialogFooter>
            <AlertDialogCancel disabled={busy}>Keep it</AlertDialogCancel>
            <Button
              variant="destructive"
              disabled={busy}
              onClick={() => {
                if (removal) void save(removal.command)
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

function wishCount(count: number): string {
  return `${count} ${count === 1 ? "wish" : "wishes"}`
}

function AddWish({
  ownerId,
  listId,
  disabled,
  save,
  metadata,
}: {
  ownerId: string
  listId: string
  disabled: boolean
  save: (command: WishlistCommand) => Promise<boolean>
  metadata: WishlistMetadataLoader
}): React.JSX.Element {
  const pending = useRef<PendingAddition | undefined>(undefined)
  const [restored, setRestored] = useState<PendingAddition>()

  // Browser storage is read after hydration; server rendering stays deterministic.
  useEffect(() => {
    const recovered = readPendingAddition(listId)

    pending.current = recovered
    // eslint-disable-next-line react/set-state-in-effect -- Synchronize a browser storage retry intention after hydration; SSR has no sessionStorage.
    setRestored(recovered)
  }, [listId])

  return (
    <ItemForm
      key={restored?.requestId ?? "fresh"}
      initialValue={
        restored?.details ?? (restored ? { title: restored.title } : undefined)
      }
      label="Something you’d like"
      submitLabel="Add wish"
      disabled={disabled}
      loadMetadata={(url) =>
        metadata({ owner_id: ownerId, list_id: listId, url })
      }
      save={async (details) => {
        const title = details.title
        pending.current ??= readPendingAddition(listId)

        if (
          !pending.current ||
          trimWishlistText(pending.current.title) !== trimWishlistText(title) ||
          pending.current.ownerId !== ownerId ||
          JSON.stringify(
            pending.current.details ?? {
              title: trimWishlistText(pending.current.title),
              url: null,
              image_url: null,
              notes: null,
              price: null,
              quantity: 1,
              priority: "normal",
            }
          ) !== JSON.stringify(details)
        )
          pending.current = {
            ownerId,
            listId,
            title,
            details,
            requestId: createRequestId(),
          }

        storePendingAddition(listId, pending.current)

        const saved = await save({
          kind: "add-item",
          owner_id: ownerId,
          list_id: listId,
          ...details,
          request_id: pending.current.requestId,
        })

        if (saved) {
          pending.current = undefined
          setRestored(undefined)
          storePendingAddition(listId, undefined)
        }

        return saved
      }}
    />
  )
}
