import { useId } from "react"
import { cn } from "@/lib/utils"
import { Input } from "@/components/ui/input"
import { Textarea } from "@/components/ui/textarea"
import { Segments } from "@/components/list-parts"
import { Stepper, field } from "@/components/sheet-parts"
import { trimText } from "@/lib/lists"
import type { WishlistDetails, WishlistItem } from "@/lib/wishlist"

type Priority = WishlistItem["priority"]

export type WishDraft = {
  title: string
  url: string
  image_url: string
  notes: string
  price: string
  quantity: number
  priority: Priority
}

export const emptyWish: WishDraft = {
  title: "",
  url: "",
  image_url: "",
  notes: "",
  price: "",
  quantity: 1,
  priority: "normal",
}

export function draftOf(item: WishlistItem): WishDraft {
  return {
    title: item.title,
    url: item.url ?? "",
    image_url: item.image_url ?? "",
    notes: item.notes ?? "",
    price: item.price ?? "",
    quantity: item.quantity,
    priority: item.priority,
  }
}

export function detailsOf(draft: WishDraft): Required<WishlistDetails> {
  const optional = (value: string): string | null => trimText(value) || null

  return {
    title: trimText(draft.title),
    url: optional(draft.url),
    image_url: optional(draft.image_url),
    notes: optional(draft.notes),
    price: optional(draft.price),
    quantity: draft.quantity,
    priority: draft.priority,
  }
}

const priorities = [
  { value: "low", label: "Low" },
  { value: "normal", label: "Normal" },
  { value: "high", label: "High" },
] as const satisfies ReadonlyArray<{ value: Priority; label: string }>

export function WishFields({
  draft,
  edit,
  disabled,
  withLink,
}: {
  draft: WishDraft
  edit: (change: Partial<WishDraft>) => void
  disabled: boolean
  withLink: boolean
}): React.JSX.Element {
  const id = useId()

  return (
    <div className="grid min-w-0 gap-4">
      <label className={field({ emphasis: "title" })} htmlFor={`${id}-title`}>
        <span>Title</span>
        <Input
          id={`${id}-title`}
          name="title"
          value={draft.title}
          onChange={(event) => edit({ title: event.target.value })}
          required
          maxLength={500}
          disabled={disabled}
          autoComplete="off"
        />
      </label>
      <div className="grid grid-cols-trail gap-4">
        <label className={cn(field(), "num")} htmlFor={`${id}-price`}>
          <span>Price</span>
          <Input
            id={`${id}-price`}
            name="price"
            value={draft.price}
            onChange={(event) => edit({ price: event.target.value })}
            maxLength={100}
            disabled={disabled}
            placeholder="249 kr"
            autoComplete="off"
          />
        </label>
        <div className={field()}>
          <span id={`${id}-quantity`}>Quantity</span>
          <Stepper
            labelId={`${id}-quantity`}
            value={draft.quantity}
            disabled={disabled}
            max={999}
            set={(quantity) => edit({ quantity })}
          />
        </div>
      </div>
      <div className={field()}>
        <span>Priority</span>
        <Segments
          label="Priority"
          value={draft.priority}
          options={priorities}
          choose={(priority) => edit({ priority })}
          disabled={disabled}
        />
      </div>
      {withLink ? (
        <label className={field()} htmlFor={`${id}-url`}>
          <span>Link</span>
          <Input
            id={`${id}-url`}
            name="url"
            type="url"
            value={draft.url}
            onChange={(event) => edit({ url: event.target.value })}
            maxLength={2048}
            disabled={disabled}
            placeholder="https://…"
          />
        </label>
      ) : null}
      <label className={field()} htmlFor={`${id}-image`}>
        <span>Image link</span>
        <Input
          id={`${id}-image`}
          name="image_url"
          type="url"
          value={draft.image_url}
          onChange={(event) => edit({ image_url: event.target.value })}
          maxLength={2048}
          disabled={disabled}
          placeholder="https://…"
        />
      </label>
      <label className={field()} htmlFor={`${id}-notes`}>
        <span>Notes</span>
        <Textarea
          id={`${id}-notes`}
          name="notes"
          value={draft.notes}
          onChange={(event) => edit({ notes: event.target.value })}
          maxLength={5000}
          disabled={disabled}
          rows={3}
        />
      </label>
    </div>
  )
}
