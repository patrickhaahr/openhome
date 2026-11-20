import { useState } from "react"
import { ArrowUpRightIcon } from "@phosphor-icons/react/dist/csr/ArrowUpRight"
import { PencilSimpleIcon } from "@phosphor-icons/react/dist/csr/PencilSimple"
import { TrashIcon } from "@phosphor-icons/react/dist/csr/Trash"
import { cva, type VariantProps } from "class-variance-authority"
import { Button } from "@/components/ui/button"
import { WishMark } from "@/components/list-frame"
import { PersonDisc, useSheetLifetime } from "@/components/list-parts"
import {
  sheetActions,
  sheetBody,
  sheetConfirm,
  sheetLink,
  sheetName,
  sheetNotes,
  sheetOwner,
} from "@/components/sheet-parts"
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

const art = cva("relative block overflow-hidden bg-wash-strong", {
  variants: {
    size: {
      plate: "aspect-square",
      detail: "h-detail-art rounded-plate",
      preview: "aspect-square rounded-art max-sm:w-28",
    },
  },
  defaultVariants: { size: "plate" },
})

const artType = cva(
  "absolute inset-0 flex items-end overflow-hidden leading-poster font-semibold tracking-headline text-balance text-ink",
  {
    variants: {
      size: {
        plate: "p-4 text-art max-sm:p-3 max-sm:text-xlarge",
        detail: "p-4 text-art max-sm:p-3 max-sm:text-xlarge",
        preview: "p-3 text-large",
      },
    },
    defaultVariants: { size: "plate" },
  }
)

export function WishArt({
  title,
  image,
  size,
}: {
  title: string
  image: string | null
} & VariantProps<typeof art>): React.JSX.Element {
  return (
    <span className={art({ size })}>
      <span className={artType({ size })} aria-hidden="true">
        {title}
      </span>
      {image === null ? null : (
        <img
          key={image}
          src={image}
          alt=""
          loading="lazy"
          decoding="async"
          className="absolute inset-0 size-full object-cover transition-transform duration-mid ease-app group-hover/plate:scale-103"
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
  const alive = useSheetLifetime()
  const [draft, setDraft] = useState<WishDraft>(() => draftOf(item))
  const target = { owner_id: owner.id, list_id: list.id, item_id: item.id }

  if (mode === "edit") {
    const original = detailsOf(draftOf(item))
    const next = detailsOf(draft)

    return (
      <form
        className={sheetBody()}
        aria-label={`Edit ${item.title}`}
        onSubmit={(event) => {
          event.preventDefault()

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
        <div className={sheetActions()}>
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
    <div className={sheetBody()}>
      <div className="relative">
        <WishArt
          title={item.title}
          image={wishImageUrl(owner.id, item)}
          size="detail"
        />
        {item.priority === "high" ? <WishMark /> : null}
      </div>
      <div className="grid gap-2.5">
        <p className={sheetOwner}>
          <PersonDisc name={owner.name} />
          {owner.name}
          <span aria-hidden="true">·</span>
          {list.name}
        </p>
        <p className={sheetName} aria-hidden="true">
          {item.title}
        </p>
      </div>
      <dl className="flex flex-wrap items-baseline gap-x-6 gap-y-2">
        <div>
          <dt className="sr-only">Price</dt>
          <dd
            className="text-figure-sm font-medium tracking-figure-tight tabular-nums data-[gap=true]:gap-dash-7"
            data-gap={item.price === null}
          >
            {item.price ?? "Unavailable"}
          </dd>
        </div>
        <div>
          <dt className="sr-only">Quantity</dt>
          <dd className="text-figure-sm font-medium tracking-figure-tight tabular-nums">
            <span aria-hidden="true">×</span>
            {item.quantity}
          </dd>
        </div>
        <div>
          <dt className="sr-only">Priority</dt>
          <dd
            className="inline-flex items-center gap-2 text-muted-ink data-[priority=high]:before:size-3 data-[priority=high]:before:rounded-full data-[priority=high]:before:bg-sun"
            data-priority={item.priority}
          >
            {item.priority === "high"
              ? "High priority"
              : item.priority === "low"
                ? "Low priority"
                : "Normal priority"}
          </dd>
        </div>
      </dl>
      {item.notes === null ? null : (
        <p className={sheetNotes()}>{item.notes}</p>
      )}
      {item.url === null ? null : (
        <a
          className={sheetLink}
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
        <div className={sheetConfirm} role="group" aria-label="Delete wish">
          <p>
            Delete “{item.title}” from {list.name}? This cannot be undone.
          </p>
          <div className={sheetActions()}>
            <Button
              variant="destructive"
              size="key"
              disabled={busy}
              onClick={() => {
                void save(
                  { kind: "remove-item", ...target },
                  "Wish deleted."
                ).then((saved) => {
                  if (saved && alive()) done()
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
        <div className={sheetActions({ placement: "end" })}>
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
