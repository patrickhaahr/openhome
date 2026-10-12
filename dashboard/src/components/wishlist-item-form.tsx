import { useId, useState } from "react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Textarea } from "@/components/ui/textarea"
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select"
import type { WishlistDetails } from "@/lib/wishlist"

const fields = [
  "title",
  "url",
  "image_url",
  "notes",
  "price",
  "quantity",
  "priority",
] as const

function itemDraft(item?: WishlistDetails) {
  return {
    title: item?.title ?? "",
    url: item?.url ?? "",
    image_url: item?.image_url ?? "",
    notes: item?.notes ?? "",
    price: item?.price ?? "",
    quantity: String(item?.quantity ?? 1),
    priority: item?.priority ?? "normal",
  }
}

/** Manual rich-item entry; failed writes keep the draft and Axum validates the fields. */
export function ItemForm({
  label,
  initialValue,
  submitLabel,
  disabled,
  save,
  cancel,
}: {
  label: string
  initialValue?: WishlistDetails | undefined
  submitLabel: string
  disabled: boolean
  save: (details: WishlistDetails) => Promise<boolean>
  cancel?: () => void
}): React.JSX.Element {
  const id = useId()
  const source = itemDraft(initialValue)
  const [draft, setDraft] = useState(() => ({ source, values: source }))

  // Refreshed server values replace pristine fields; unsaved edits remain intact.
  if (fields.some((field) => source[field] !== draft.source[field])) {
    const values = { ...draft.values }

    for (const field of fields)
      if (values[field] === draft.source[field]) values[field] = source[field]

    setDraft({ source, values })
  }

  function edit(field: (typeof fields)[number], value: string): void {
    setDraft((current) => ({
      source: current.source,
      values: { ...current.values, [field]: value },
    }))
  }

  return (
    <form
      className="wishlist-item-form"
      aria-label={submitLabel}
      onSubmit={(event) => {
        event.preventDefault()

        // Match Axum's Unicode whitespace trimming for stable retry arguments.
        const text = (name: (typeof fields)[number]): string =>
          draft.values[name].replace(
            /^\p{White_Space}+|\p{White_Space}+$/gu,
            ""
          )

        const optional = (name: (typeof fields)[number]): string | null =>
          text(name) || null

        void save({
          title: text("title"),
          url: optional("url"),
          image_url: optional("image_url"),
          notes: optional("notes"),
          price: optional("price"),
          quantity: Number(draft.values.quantity),
          priority: text("priority"),
        }).then((saved) => {
          if (saved && initialValue === undefined)
            setDraft({ source, values: itemDraft() })
        })
      }}
    >
      <label htmlFor={`${id}-title`}>{label}</label>
      <div className="wishlist-form-row">
        <Input
          id={`${id}-title`}
          name="title"
          value={draft.values.title}
          onChange={(event) => edit("title", event.target.value)}
          required
          maxLength={500}
          disabled={disabled}
          autoComplete="off"
        />
        <Button type="submit" disabled={disabled}>
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
      <details
        open={initialValue !== undefined}
        className="wishlist-item-fields"
      >
        <summary>
          Item details <span>Links, notes, quantity and priority</span>
        </summary>
        <div className="wishlist-field-grid">
          <div>
            <label htmlFor={`${id}-url`}>Product URL</label>
            <Input
              id={`${id}-url`}
              name="url"
              type="url"
              value={draft.values.url}
              onChange={(event) => edit("url", event.target.value)}
              maxLength={2048}
              disabled={disabled}
              placeholder="https://…"
            />
          </div>
          <div>
            <label htmlFor={`${id}-image`}>Image URL</label>
            <Input
              id={`${id}-image`}
              name="image_url"
              type="url"
              value={draft.values.image_url}
              onChange={(event) => edit("image_url", event.target.value)}
              maxLength={2048}
              disabled={disabled}
              placeholder="https://…"
            />
          </div>
          <div>
            <label htmlFor={`${id}-price`}>Price (include currency)</label>
            <Input
              id={`${id}-price`}
              name="price"
              value={draft.values.price}
              onChange={(event) => edit("price", event.target.value)}
              maxLength={100}
              disabled={disabled}
              placeholder="DKK 199.95"
            />
          </div>
          <div className="wishlist-field-pair">
            <div>
              <label htmlFor={`${id}-quantity`}>Quantity</label>
              <Input
                id={`${id}-quantity`}
                name="quantity"
                type="number"
                min={1}
                max={999}
                step={1}
                value={draft.values.quantity}
                onChange={(event) => edit("quantity", event.target.value)}
                required
                disabled={disabled}
              />
            </div>
            <div>
              <label htmlFor={`${id}-priority`}>Priority</label>
              <NativeSelect
                id={`${id}-priority`}
                name="priority"
                value={draft.values.priority}
                onChange={(event) => edit("priority", event.target.value)}
                disabled={disabled}
              >
                <NativeSelectOption value="low">Low</NativeSelectOption>
                <NativeSelectOption value="normal">Normal</NativeSelectOption>
                <NativeSelectOption value="high">High</NativeSelectOption>
              </NativeSelect>
            </div>
          </div>
          <div className="wishlist-field-notes">
            <label htmlFor={`${id}-notes`}>Notes</label>
            <Textarea
              id={`${id}-notes`}
              name="notes"
              value={draft.values.notes}
              onChange={(event) => edit("notes", event.target.value)}
              maxLength={5000}
              disabled={disabled}
              rows={3}
            />
          </div>
        </div>
      </details>
    </form>
  )
}
