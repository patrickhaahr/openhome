import { useId, useState } from "react"
import { CheckIcon } from "@phosphor-icons/react/dist/csr/Check"
import { PlusIcon } from "@phosphor-icons/react/dist/csr/Plus"
import { TrashIcon } from "@phosphor-icons/react/dist/csr/Trash"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { PersonDisc } from "@/components/list-parts"
import { trimText } from "@/lib/lists"
import type { HouseholdUser, Wishlist, WishlistCommand } from "@/lib/wishlist"

type Save = (command: WishlistCommand, saved: string) => Promise<boolean>

/** One name with a save key; removal asks once, inline. */
function NameRow({
  label,
  name,
  busy,
  rename,
  remove,
  removal,
}: {
  label: string
  name: string
  busy: boolean
  rename: (name: string) => Promise<boolean>
  remove: () => Promise<boolean>
  removal: string
}): React.JSX.Element {
  const id = useId()
  const [value, setValue] = useState(name)
  const [confirming, setConfirming] = useState(false)
  const changed = trimText(value) !== name && trimText(value) !== ""

  return (
    <div className="name-row">
      <form
        className="name-form"
        onSubmit={(event) => {
          event.preventDefault()

          if (changed) void rename(trimText(value))
        }}
      >
        <label className="sr-only" htmlFor={id}>
          {label}
        </label>
        <Input
          id={id}
          value={value}
          maxLength={200}
          disabled={busy}
          onChange={(event) => setValue(event.target.value)}
        />
        {changed ? (
          <Button
            type="submit"
            variant="outline"
            size="key-icon"
            disabled={busy}
          >
            <CheckIcon size={18} aria-hidden="true" />
            <span className="sr-only">Save {label.toLowerCase()}</span>
          </Button>
        ) : null}
        <Button
          type="button"
          variant="ghost"
          size="key-icon"
          disabled={busy}
          onClick={() => setConfirming(true)}
        >
          <TrashIcon size={18} aria-hidden="true" />
          <span className="sr-only">Remove {name}</span>
        </Button>
      </form>
      {confirming ? (
        <div
          className="sheet-confirm"
          role="group"
          aria-label={`Remove ${name}`}
        >
          <p>{removal}</p>
          <div className="sheet-actions">
            <Button
              variant="destructive"
              disabled={busy}
              onClick={() => void remove()}
            >
              Remove for good
            </Button>
            <Button
              variant="ghost"
              disabled={busy}
              onClick={() => setConfirming(false)}
            >
              Keep
            </Button>
          </div>
        </div>
      ) : null}
    </div>
  )
}

/** A name field with an add key, cleared after a confirmed save. */
function AddRow({
  label,
  busy,
  add,
}: {
  label: string
  busy: boolean
  add: (name: string) => Promise<boolean>
}): React.JSX.Element {
  const id = useId()
  const [value, setValue] = useState("")

  return (
    <form
      className="name-form"
      onSubmit={(event) => {
        event.preventDefault()

        void add(trimText(value)).then((saved) => {
          if (saved) setValue("")
        })
      }}
    >
      <label className="sr-only" htmlFor={id}>
        {label}
      </label>
      <Input
        id={id}
        value={value}
        maxLength={200}
        disabled={busy}
        placeholder={label}
        onChange={(event) => setValue(event.target.value)}
      />
      <Button
        type="submit"
        variant="outline"
        size="key-icon"
        disabled={busy || trimText(value) === ""}
      >
        <PlusIcon size={18} aria-hidden="true" />
        <span className="sr-only">{label}</span>
      </Button>
    </form>
  )
}

/** The household and each person's named lists: rename, remove, add. */
export function HouseholdLists({
  owners,
  lists,
  counts,
  busy,
  save,
}: {
  owners: readonly HouseholdUser[]
  lists: readonly Wishlist[]
  counts: ReadonlyMap<string, number>
  busy: boolean
  save: Save
}): React.JSX.Element {
  return (
    <div className="sheet-body household">
      {owners.map((owner) => (
        <section
          key={owner.id}
          className="household-person"
          aria-label={owner.name}
        >
          <div className="household-name">
            <PersonDisc name={owner.name} />
            <NameRow
              label="Name"
              name={owner.name}
              busy={busy}
              rename={(name) =>
                save(
                  { kind: "rename-owner", owner_id: owner.id, name },
                  `Renamed to ${name}.`
                )
              }
              remove={() =>
                save(
                  { kind: "remove-owner", owner_id: owner.id },
                  `Removed ${owner.name}.`
                )
              }
              removal={`Remove ${owner.name} with all their wishlists, wishes and watchlist entries? This cannot be undone.`}
            />
          </div>
          <div className="household-lists">
            {lists
              .filter((list) => list.owner_id === owner.id)
              .map((list) => {
                const count = counts.get(list.id) ?? 0

                return (
                  <NameRow
                    key={list.id}
                    label="List name"
                    name={list.name}
                    busy={busy}
                    rename={(name) =>
                      save(
                        {
                          kind: "rename-list",
                          owner_id: owner.id,
                          list_id: list.id,
                          name,
                        },
                        `Renamed the list to ${name}.`
                      )
                    }
                    remove={() =>
                      save(
                        {
                          kind: "remove-list",
                          owner_id: owner.id,
                          list_id: list.id,
                        },
                        `Removed the list ${list.name}.`
                      )
                    }
                    removal={`Remove ${owner.name}’s ${list.name} and the ${count} ${count === 1 ? "wish" : "wishes"} in it? This cannot be undone.`}
                  />
                )
              })}
            <AddRow
              label={`New list for ${owner.name}`}
              busy={busy}
              add={(name) =>
                save(
                  { kind: "create-list", owner_id: owner.id, name },
                  `Created the list ${name}.`
                )
              }
            />
          </div>
        </section>
      ))}
      <AddRow
        label="New household member"
        busy={busy}
        add={(name) => save({ kind: "create-owner", name }, `Added ${name}.`)}
      />
    </div>
  )
}
