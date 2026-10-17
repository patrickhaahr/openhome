import { useState } from "react"
import { ArrowUpRightIcon } from "@phosphor-icons/react/dist/csr/ArrowUpRight"
import { PencilSimpleIcon } from "@phosphor-icons/react/dist/csr/PencilSimple"
import { TrashIcon } from "@phosphor-icons/react/dist/csr/Trash"
import { Button } from "@/components/ui/button"
import { PersonDisc } from "@/components/list-parts"
import {
  WishFields,
  detailsOf,
  draftOf,
  type WishDraft,
} from "@/components/wish-fields"
import {
  wishImageUrl,
  type HouseholdUser,
  type Wishlist,
  type WishlistCommand,
  type WishlistItem,
} from "@/lib/wishlist"

/** The product image with the title as its fallback poster; a failed image reveals the title. */
export function WishArt({
  title,
  image,
}: {
  title: string
  image: string | null
}): React.JSX.Element {
  return (
    <span className="wish-art">
      <span className="wish-art-type" aria-hidden="true">
        {title}
      </span>
      {image === null ? null : (
        <img
          key={image}
          src={image}
          alt=""
          loading="lazy"
          decoding="async"
          referrerPolicy="no-referrer"
          onError={(event) => {
            event.currentTarget.hidden = true
          }}
          onLoad={(event) => {
            event.currentTarget.hidden = false
          }}
        />
      )}
    </span>
  )
}

function host(url: string): string {
  try {
    return new URL(url).host.replace(/^www\./, "")
  } catch {
    return url
  }
}

/** Everything about one wish, its edit form, and its delete with an inline confirmation. */
export function WishDetail({
  item,
  list,
  owner,
  busy,
  save,
  done,
}: {
  item: WishlistItem
  list: Wishlist
  owner: HouseholdUser
  busy: boolean
  save: (command: WishlistCommand, saved: string) => Promise<boolean>
  done: () => void
}): React.JSX.Element {
  const [mode, setMode] = useState<"view" | "edit" | "delete">("view")
  const [draft, setDraft] = useState<WishDraft>(() => draftOf(item))
  const target = { owner_id: owner.id, list_id: list.id, item_id: item.id }

  if (mode === "edit") {
    const original = detailsOf(draftOf(item))
    const next = detailsOf(draft)

    return (
      <form
        className="sheet-body"
        aria-label={`Edit ${item.title}`}
        onSubmit={(event) => {
          event.preventDefault()

          // Only changed fields travel; omitted fields stay as stored.
          void save(
            {
              kind: "edit-item",
              ...target,
              title: next.title === original.title ? undefined : next.title,
              url: next.url === original.url ? undefined : next.url,
              image_url:
                next.image_url === original.image_url
                  ? undefined
                  : next.image_url,
              notes: next.notes === original.notes ? undefined : next.notes,
              price: next.price === original.price ? undefined : next.price,
              quantity:
                next.quantity === original.quantity ? undefined : next.quantity,
              priority:
                next.priority === original.priority ? undefined : next.priority,
            },
            "Wish saved."
          ).then((saved) => {
            if (saved) setMode("view")
          })
        }}
      >
        <WishFields
          draft={draft}
          edit={(change) => setDraft((current) => ({ ...current, ...change }))}
          disabled={busy}
          withLink
        />
        <div className="sheet-actions">
          <Button type="submit" size="key" disabled={busy || next.title === ""}>
            Save
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="key"
            disabled={busy}
            onClick={() => {
              setDraft(draftOf(item))
              setMode("view")
            }}
          >
            Cancel
          </Button>
        </div>
      </form>
    )
  }

  return (
    <div className="sheet-body">
      <div className="wish-detail-art">
        <WishArt title={item.title} image={wishImageUrl(owner.id, item)} />
        {item.priority === "high" ? (
          <i className="wish-mark" aria-hidden="true" />
        ) : null}
      </div>
      <div className="wish-detail-head">
        <p className="sheet-owner">
          <PersonDisc name={owner.name} />
          {owner.name}
          <span aria-hidden="true">·</span>
          {list.name}
        </p>
        {/* The sheet's title already names the wish for assistive technology. */}
        <p className="sheet-name" aria-hidden="true">
          {item.title}
        </p>
      </div>
      <dl className="sheet-figures">
        <div>
          <dt className="sr-only">Price</dt>
          <dd className="num" data-gap={item.price === null}>
            {item.price ?? "Unavailable"}
          </dd>
        </div>
        <div>
          <dt className="sr-only">Quantity</dt>
          <dd className="num">
            <span aria-hidden="true">×</span>
            {item.quantity}
          </dd>
        </div>
        <div>
          <dt className="sr-only">Priority</dt>
          <dd className="sheet-priority" data-priority={item.priority}>
            {item.priority === "high"
              ? "High priority"
              : item.priority === "low"
                ? "Low priority"
                : "Normal priority"}
          </dd>
        </div>
      </dl>
      {item.notes === null ? null : <p className="sheet-notes">{item.notes}</p>}
      {item.url === null ? null : (
        <a
          className="sheet-link"
          href={item.url}
          target="_blank"
          rel="noreferrer"
        >
          {host(item.url)}
          <ArrowUpRightIcon size={18} aria-hidden="true" />
          <span className="sr-only"> (opens the product page)</span>
        </a>
      )}
      {mode === "delete" ? (
        <div className="sheet-confirm" role="group" aria-label="Delete wish">
          <p>
            Delete “{item.title}” from {list.name}? This cannot be undone.
          </p>
          <div className="sheet-actions">
            <Button
              variant="destructive"
              size="key"
              disabled={busy}
              onClick={() => {
                void save(
                  { kind: "remove-item", ...target },
                  "Wish deleted."
                ).then((saved) => {
                  if (saved) done()
                })
              }}
            >
              <TrashIcon size={18} aria-hidden="true" />
              Delete for good
            </Button>
            <Button
              variant="ghost"
              size="key"
              disabled={busy}
              onClick={() => setMode("view")}
            >
              Keep it
            </Button>
          </div>
        </div>
      ) : (
        <div className="sheet-actions sheet-actions-end">
          <Button
            variant="outline"
            size="key"
            disabled={busy}
            onClick={() => {
              setDraft(draftOf(item))
              setMode("edit")
            }}
          >
            <PencilSimpleIcon size={18} aria-hidden="true" />
            Edit
          </Button>
          <Button
            variant="destructive"
            size="key"
            disabled={busy}
            onClick={() => setMode("delete")}
          >
            <TrashIcon size={18} aria-hidden="true" />
            Delete
          </Button>
        </div>
      )}
    </div>
  )
}
