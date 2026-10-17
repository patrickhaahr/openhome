import { useId } from "react"
import { MinusIcon } from "@phosphor-icons/react/dist/csr/Minus"
import { PlusIcon } from "@phosphor-icons/react/dist/csr/Plus"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Textarea } from "@/components/ui/textarea"
import { Segments } from "@/components/list-parts"
import { trimText } from "@/lib/lists"
import type { WishlistDetails, WishlistItem } from "@/lib/wishlist"

type Priority = WishlistItem["priority"]

/** Every editable field of a wish as typed; Axum owns the limits and URL rules. */
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

/** Trimmed write arguments, stable across retries; blank optional fields are null. */
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

/** Title first and large; the rest as quiet plates. No selects: priority is three keys. */
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
    <div className="wish-fields">
      <label className="field field-title" htmlFor={`${id}-title`}>
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
      <div className="field-row">
        <label className="field num" htmlFor={`${id}-price`}>
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
        <div className="field">
          <span id={`${id}-quantity`}>Quantity</span>
          <div
            className="stepper"
            role="group"
            aria-labelledby={`${id}-quantity`}
          >
            <Button
              type="button"
              variant="outline"
              size="key-icon"
              disabled={disabled || draft.quantity <= 1}
              onClick={() => edit({ quantity: draft.quantity - 1 })}
            >
              <MinusIcon size={18} aria-hidden="true" />
              <span className="sr-only">One fewer</span>
            </Button>
            <output className="num" aria-live="polite">
              {draft.quantity}
            </output>
            <Button
              type="button"
              variant="outline"
              size="key-icon"
              disabled={disabled || draft.quantity >= 999}
              onClick={() => edit({ quantity: draft.quantity + 1 })}
            >
              <PlusIcon size={18} aria-hidden="true" />
              <span className="sr-only">One more</span>
            </Button>
          </div>
        </div>
      </div>
      <div className="field">
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
        <label className="field" htmlFor={`${id}-url`}>
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
      <label className="field" htmlFor={`${id}-image`}>
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
      <label className="field" htmlFor={`${id}-notes`}>
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
