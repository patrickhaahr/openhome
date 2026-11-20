import { useId, useRef, useState } from "react"
import { TrashIcon } from "@phosphor-icons/react/dist/csr/Trash"
import { CardValueLine } from "@/components/binder-value"
import { PriceHistory } from "@/components/price-history"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Textarea } from "@/components/ui/textarea"
import { PersonDisc, Segments, useSheetLifetime } from "@/components/list-parts"
import {
  factTerm,
  field,
  mediaMeta,
  sheetActions,
  sheetConfirm,
  sheetOwner,
} from "@/components/sheet-parts"
import { cardImageUrl, type BinderEntry } from "@/lib/binder"
import type { PricesState } from "@/lib/card-prices"
import { maxQuantity, type BinderCommand } from "@/lib/binder-edit"
import { swipeStep } from "@/lib/binder-turn"
import { formatDay } from "@/lib/lists"
import { cn } from "@/lib/utils"
import type { HouseholdUser } from "@/lib/wishlist"

const detailColumn =
  "grid min-w-0 content-start gap-7 binder-stage-wide:row-start-1 binder-stage-wide:overflow-y-auto binder-stage-wide:overscroll-contain"

type Save = (command: BinderCommand, saved: string) => Promise<boolean>

const languages = [
  { value: "en", label: "EN" },
  { value: "ja", label: "JP" },
] as const satisfies ReadonlyArray<{
  value: BinderEntry["language"]
  label: string
}>

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

export function BinderSheet({
  entry,
  owner,
  busy,
  save,
  prices,
  step,
  done,
}: {
  entry: BinderEntry
  owner: HouseholdUser
  busy: boolean
  save: Save
  prices: (card: string) => Promise<PricesState>
  step: (offset: -1 | 1) => void
  done: () => void
}): React.JSX.Element {
  const [removing, setRemoving] = useState(false)
  const press = useRef<{ pointer: number; x: number; y: number }>(undefined)
  const alive = useSheetLifetime()
  const { card } = entry

  return (
    <div
      data-slot="binder-detail"
      className="grid min-h-0 touch-pan-y grid-cols-lead gap-binder-detail-gap px-binder-stage-inset pb-gutter binder-stage-wide:grid-cols-binder-turn binder-stage-wide:gap-x-binder-detail-wide-gap binder-stage-portrait:auto-rows-max binder-stage-portrait:grid-cols-1 binder-stage-portrait:content-start binder-stage-portrait:overflow-y-auto binder-stage-portrait:overscroll-contain"
      onPointerDown={(event) => {
        press.current =
          event.isPrimary &&
          event.button === 0 &&
          !(
            event.target instanceof Element &&
            event.target.closest("input, textarea, select") !== null
          )
            ? { pointer: event.pointerId, x: event.clientX, y: event.clientY }
            : undefined
      }}
      onPointerUp={(event) => {
        const start = press.current
        press.current = undefined

        if (
          start?.pointer !== event.pointerId ||
          window.getSelection()?.isCollapsed === false
        )
          return

        const offset = swipeStep(
          event.clientX - start.x,
          event.clientY - start.y
        )

        if (offset !== 0) step(offset)
      }}
      onPointerCancel={() => {
        press.current = undefined
      }}
    >
      <div className="relative aspect-binder-card h-full max-w-binder-art overflow-hidden rounded-binder-art bg-wash-strong shadow-binder-card binder-stage-wide:col-start-2 binder-stage-wide:row-start-1 binder-stage-portrait:h-auto binder-stage-portrait:w-binder-art-portrait binder-stage-portrait:max-w-full binder-stage-portrait:justify-self-center">
        <img
          className="absolute inset-0 size-full rounded-binder-art object-cover outline -outline-offset-1 outline-binder-hairline"
          draggable={false}
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
      </div>
      <div className="grid max-w-176 min-w-0 content-start gap-7 overflow-y-auto overscroll-contain pb-4 binder-stage-wide:contents binder-stage-portrait:max-w-none binder-stage-portrait:overflow-visible">
        <div className={cn(detailColumn, "binder-stage-wide:col-start-1")}>
          <div className="grid gap-2">
            <p className={sheetOwner}>
              <PersonDisc name={owner.name} />
              {owner.name}
            </p>
            <p className={mediaMeta}>
              <span className="num">{card.card_code}</span>
              <span>
                Added{" "}
                <time dateTime={entry.date_added}>
                  {formatDay(entry.date_added)}
                </time>
              </span>
            </p>
          </div>
          <CardValueLine card={card} />
          <dl
            data-slot="binder-facts"
            className="grid grid-fill-34 gap-x-6 gap-y-4"
          >
            {factsOf(card).map(([label, value]) => (
              <div key={label}>
                <dt className={factTerm}>{label}</dt>
                <dd className="font-medium">{value}</dd>
              </div>
            ))}
          </dl>
          {card.text === null ? null : (
            <p className="rounded-plate bg-wash px-5 py-4 text-body-lg leading-normal whitespace-pre-line">
              {card.text}
            </p>
          )}
        </div>
        <div className={cn(detailColumn, "binder-stage-wide:col-start-3")}>
          {entry.language === "en" ? (
            <PriceHistory card={card.id} load={prices} />
          ) : null}
          <EntryFields
            key={`${entry.quantity}-${entry.language}-${entry.note ?? ""}`}
            entry={entry}
            owner={owner}
            busy={busy}
            save={save}
          />
          {removing ? (
            <div className={sheetConfirm} role="group" aria-label="Remove card">
              <p>
                Remove “{card.name}” ({entry.language === "ja" ? "JP" : "EN"})
                from {owner.name}’s Binder? This cannot be undone.
              </p>
              <div className={sheetActions()}>
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
            <div className={sheetActions({ placement: "end" })}>
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
      </div>
    </div>
  )
}

const copiesPattern = /^[1-9]\d{0,9}$/

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
      className="grid gap-4"
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
      <div className="grid grid-cols-trail gap-4">
        <label className={cn(field(), "num")} htmlFor={`${id}-quantity`}>
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
        <div className={field()}>
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
      <label className={field()} htmlFor={`${id}-note`}>
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
      <div className={sheetActions()}>
        <Button type="submit" size="key" disabled={busy || !dirty || !valid}>
          Save changes
        </Button>
      </div>
    </form>
  )
}
