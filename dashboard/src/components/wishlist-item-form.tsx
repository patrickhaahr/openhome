import { useId } from "react"
import { z } from "zod"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Textarea } from "@/components/ui/textarea"
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select"
import type { WishlistDetails } from "@/lib/wishlist"

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

  return (
    <form
      className="wishlist-item-form"
      aria-label={submitLabel}
      onSubmit={(event) => {
        event.preventDefault()
        const form = event.currentTarget
        const values = new FormData(form)

        // Match Axum's Unicode whitespace trimming for stable retry arguments.
        const text = (name: string): string => {
          const value = z.string().parse(values.get(name) ?? "")

          return value.replace(/^\p{White_Space}+|\p{White_Space}+$/gu, "")
        }

        const optional = (name: string): string | null => text(name) || null

        void save({
          title: text("title"),
          url: optional("url"),
          image_url: optional("image_url"),
          notes: optional("notes"),
          price: optional("price"),
          quantity: Number(values.get("quantity")),
          priority: text("priority"),
        }).then((saved) => {
          if (saved && initialValue === undefined) form.reset()
        })
      }}
    >
      <label htmlFor={`${id}-title`}>{label}</label>
      <div className="wishlist-form-row">
        <Input
          id={`${id}-title`}
          name="title"
          defaultValue={initialValue?.title ?? ""}
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
              defaultValue={initialValue?.url ?? ""}
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
              defaultValue={initialValue?.image_url ?? ""}
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
              defaultValue={initialValue?.price ?? ""}
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
                defaultValue={initialValue?.quantity ?? 1}
                required
                disabled={disabled}
              />
            </div>
            <div>
              <label htmlFor={`${id}-priority`}>Priority</label>
              <NativeSelect
                id={`${id}-priority`}
                name="priority"
                defaultValue={initialValue?.priority ?? "normal"}
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
              defaultValue={initialValue?.notes ?? ""}
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
