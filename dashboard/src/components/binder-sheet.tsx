import { useId, useState } from "react"
import { TrashIcon } from "@phosphor-icons/react/dist/csr/Trash"
import { CardValueLine } from "@/components/binder-value"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Textarea } from "@/components/ui/textarea"
import { PersonDisc, Segments, useSheetLifetime } from "@/components/list-parts"
import { cardImageUrl, type BinderEntry } from "@/lib/binder"
import { maxQuantity, type BinderCommand } from "@/lib/binder-edit"
import { formatDay } from "@/lib/lists"
import type { HouseholdUser } from "@/lib/wishlist"

type Save = (command: BinderCommand, saved: string) => Promise<boolean>

const languages = [
  { value: "en", label: "EN" },
  { value: "ja", label: "JP" },
] as const satisfies ReadonlyArray<{
  value: BinderEntry["language"]
  label: string
}>

/** The facts the catalogue gave, in reading order; a fact it left out is not shown. */
function factsOf(card: BinderEntry["card"]): [string, string][] {
  const facts: [string, string | null][] = [
    ["Set", card.set_name ?? card.set_id],
    ["Rarity", card.rarity],
    ["Colour", card.color],
    ["Type", card.card_type],
    ["Cost", card.cost],
    ["Power", card.power],
    ["Counter", card.counter_amount],
    ["Life", card.life],
    ["Attribute", card.attribute],
    ["Types", card.sub_types],
  ]

  return facts.flatMap(([label, value]) =>
    value === null ? [] : [[label, value]]
  )
}

/**
 * One Binder Entry's card sheet: the large image and every detail the Binder read carries, then
 * the entry's own quantity, language and note. Removal asks first, and only a saved removal
 * closes the sheet, so a failure leaves it open beside its error.
 */
export function BinderSheet({
  entry,
  owner,
  busy,
  save,
  done,
}: {
  entry: BinderEntry
  owner: HouseholdUser
  busy: boolean
  save: Save
  done: () => void
}): React.JSX.Element {
  const [removing, setRemoving] = useState(false)
  const alive = useSheetLifetime()
  const { card } = entry

  return (
    <div className="sheet-body">
      <div className="binder-sheet-head">
        <img
          className="binder-sheet-image"
          src={cardImageUrl(card.id)}
          alt={`${card.name}, ${card.card_code}`}
          width={600}
          height={838}
          onError={(event) => {
            event.currentTarget.hidden = true
          }}
          onLoad={(event) => {
            event.currentTarget.hidden = false
          }}
        />
        <div className="binder-sheet-text">
          <p className="sheet-owner">
            <PersonDisc name={owner.name} />
            {owner.name}
          </p>
          <p className="media-result-meta">
            <span className="num">{card.card_code}</span>
            <span>
              Added{" "}
              <time dateTime={entry.date_added}>
                {formatDay(entry.date_added)}
              </time>
            </span>
          </p>
          <CardValueLine card={card} />
          <dl className="binder-facts">
            {factsOf(card).map(([label, value]) => (
              <div key={label}>
                <dt>{label}</dt>
                <dd>{value}</dd>
              </div>
            ))}
          </dl>
          {card.text === null ? null : (
            <p className="sheet-notes">{card.text}</p>
          )}
        </div>
      </div>
      <EntryFields
        key={`${entry.quantity}-${entry.language}-${entry.note ?? ""}`}
        entry={entry}
        owner={owner}
        busy={busy}
        save={save}
      />
      {removing ? (
        <div className="sheet-confirm" role="group" aria-label="Remove card">
          <p>
            Remove “{card.name}” ({entry.language === "ja" ? "JP" : "EN"}) from{" "}
            {owner.name}’s Binder? This cannot be undone.
          </p>
          <div className="sheet-actions">
            <Button
              variant="destructive"
              size="key"
              disabled={busy}
              onClick={() => {
                void save(
                  {
                    action: "remove",
                    owner_id: owner.id,
                    entry_id: entry.id,
                  },
                  `${card.name} removed.`
                ).then((saved) => {
                  if (saved && alive()) done()
                })
              }}
            >
              <TrashIcon size={18} aria-hidden="true" />
              Remove for good
            </Button>
            <Button
              variant="ghost"
              size="key"
              disabled={busy}
              onClick={() => setRemoving(false)}
            >
              Keep it
            </Button>
          </div>
        </div>
      ) : (
        <div className="sheet-actions sheet-actions-end">
          <Button
            variant="destructive"
            size="key"
            disabled={busy}
            onClick={() => setRemoving(true)}
          >
            <TrashIcon size={18} aria-hidden="true" />
            Remove
          </Button>
        </div>
      )}
    </div>
  )
}

const copiesPattern = /^[1-9]\d{0,9}$/

/**
 * The draft starts from the stored entry, and the parent remounts it whenever the stored
 * entry changes, so a saved edit shows what was read back and a failed one keeps the draft.
 */
function EntryFields({
  entry,
  owner,
  busy,
  save,
}: {
  entry: BinderEntry
  owner: HouseholdUser
  busy: boolean
  save: Save
}): React.JSX.Element {
  const id = useId()
  const [quantity, setQuantity] = useState(String(entry.quantity))
  const [language, setLanguage] = useState(entry.language)
  const [note, setNote] = useState(entry.note ?? "")
  const copies = copiesPattern.test(quantity) ? Number(quantity) : undefined
  const valid = copies !== undefined && copies <= maxQuantity
  const trimmed = note.trim()

  const change = {
    quantity: valid && copies !== entry.quantity ? copies : undefined,
    language: language === entry.language ? undefined : language,
    note:
      trimmed === (entry.note ?? "")
        ? undefined
        : trimmed === ""
          ? null
          : trimmed,
  }

  const dirty = Object.values(change).some((value) => value !== undefined)

  return (
    <form
      className="binder-fields"
      aria-label="Edit entry"
      onSubmit={(event) => {
        event.preventDefault()

        void save(
          {
            action: "edit",
            owner_id: owner.id,
            entry_id: entry.id,
            ...change,
          },
          `${entry.card.name} saved.`
        )
      }}
    >
      <div className="field-row">
        <label className="field num" htmlFor={`${id}-quantity`}>
          <span>Quantity</span>
          <Input
            id={`${id}-quantity`}
            type="number"
            inputMode="numeric"
            min={1}
            max={maxQuantity}
            step={1}
            value={quantity}
            required
            disabled={busy}
            aria-invalid={!valid}
            onChange={(event) => setQuantity(event.target.value)}
          />
        </label>
        <div className="field">
          <span id={`${id}-language`}>Language</span>
          <Segments
            label="Language"
            value={language}
            options={languages}
            disabled={busy}
            choose={setLanguage}
          />
        </div>
      </div>
      <label className="field" htmlFor={`${id}-note`}>
        <span>Note</span>
        <Textarea
          id={`${id}-note`}
          value={note}
          maxLength={500}
          rows={2}
          disabled={busy}
          onChange={(event) => setNote(event.target.value)}
        />
      </label>
      <div className="sheet-actions">
        <Button type="submit" size="key" disabled={busy || !dirty || !valid}>
          Save changes
        </Button>
      </div>
    </form>
  )
}
