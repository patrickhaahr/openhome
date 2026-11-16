import { Amount } from "@/components/food-amount"
import { useState } from "react"
import { XIcon } from "@phosphor-icons/react/dist/csr/X"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogTitle,
} from "@/components/ui/dialog"
import {
  doseCount,
  doseEnergy,
  formatAmount,
  mealPhotoUrl,
  mealTime,
  novaClassMeaning,
  portionName,
  type MealLog,
  type MealPhoto,
  type Portion,
} from "@/lib/nutrition"

export function Meal({ meal }: { readonly meal: MealLog }): React.JSX.Element {
  return (
    <article
      className="food-meal"
      data-photo={meal.photo === null ? undefined : ""}
    >
      <div className="food-meal-body">
        <MealDetails meal={meal} />
      </div>
      {meal.photo === null ? null : (
        <MealPhotoKey photo={meal.photo} time={mealTime(meal.eaten_at)} />
      )}
    </article>
  )
}

/** The meal's thumbnail; selecting it shows the full photo over the page. */
function MealPhotoKey({
  photo,
  time,
}: {
  readonly photo: MealPhoto
  readonly time: string
}): React.JSX.Element {
  const [open, setOpen] = useState(false)

  return (
    <>
      <button
        type="button"
        className="food-photo"
        aria-label={`Show the photo of the meal at ${time}`}
        onClick={() => setOpen(true)}
      >
        <img
          src={mealPhotoUrl(photo, "thumbnail")}
          width={photo.thumbnail.width}
          height={photo.thumbnail.height}
          alt=""
          loading="lazy"
          decoding="async"
        />
      </button>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent variant="fullscreen" showCloseButton={false}>
          <div className="photo-viewer food-photo-viewer">
            <DialogTitle className="sr-only">Meal at {time}</DialogTitle>
            <div className="photo-stage">
              <img
                src={mealPhotoUrl(photo, "photo")}
                width={photo.width}
                height={photo.height}
                alt={`The meal at ${time}`}
                decoding="async"
              />
            </div>
            <div className="photo-viewer-bar">
              <span className="photo-viewer-where">Meal at {time}</span>
              <DialogClose
                render={<Button variant="outline" size="key-icon" />}
                title="Close photo"
              >
                <XIcon size={22} aria-hidden="true" />
                <span className="sr-only">Close photo</span>
              </DialogClose>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </>
  )
}

function MealDetails({ meal }: { readonly meal: MealLog }): React.JSX.Element {
  return (
    <>
      <header>
        <h3>
          <time dateTime={meal.eaten_at}>{mealTime(meal.eaten_at)}</time>
        </h3>
        <Amount total={meal.totals.energy_kcal} />
      </header>
      {meal.meal_name === null ? null : (
        <p className="food-meal-from">
          <span className="sr-only">Logged from the Meal </span>
          {meal.meal_name}
        </p>
      )}
      {meal.note === null ? null : <p className="food-note">{meal.note}</p>}
      <ul className="food-portions" aria-label="Portions">
        {meal.portions.map((portion, index) => (
          <li key={`${portion.food_id}-${index}`}>
            <PortionTitle portion={portion} />
            <span className="food-portion-grams num">
              {formatAmount(portion.grams)} g
            </span>
            <span className="food-portion-kcal num">
              {portion.nutrients.energy_kcal.value === null ? (
                <span data-gap="true">
                  <span className="sr-only">Energy unknown</span>
                </span>
              ) : (
                `${formatAmount(portion.nutrients.energy_kcal.value)} kcal`
              )}
            </span>
          </li>
        ))}
      </ul>
      {meal.supplement_doses.length === 0 ? null : (
        <ul className="food-doses" aria-label="Supplement Doses">
          {meal.supplement_doses.map((dose) => {
            const kcal = doseEnergy(dose)

            return (
              <li key={dose.supplement_id}>
                <span className="food-portion-name">{dose.name}</span>
                <span className="food-portion-grams num">
                  {doseCount(dose.doses, dose.dose_unit)}
                </span>
                <span className="food-portion-kcal num">
                  {kcal === null ? null : `${formatAmount(kcal)} kcal`}
                </span>
              </li>
            )
          })}
        </ul>
      )}
    </>
  )
}

/**
 * A Food's name, or a Product's label name with its brand beneath it, and the NOVA Class it was
 * logged with; an unclassified Portion has no class mark.
 */
function PortionTitle({
  portion,
}: {
  readonly portion: Portion
}): React.JSX.Element {
  const { name, brand } = portionName(portion)

  return (
    <span className="food-portion-name">
      {name}
      {portion.nova_class === null ? null : (
        <span
          className="food-portion-nova"
          data-nova={portion.nova_class}
          title={`NOVA ${portion.nova_class}: ${novaClassMeaning[portion.nova_class]}`}
        >
          <span className="sr-only">, </span>
          NOVA {portion.nova_class}
        </span>
      )}
      {brand === null ? null : (
        <span className="food-portion-brand">
          <span className="sr-only">, </span>
          {brand}
        </span>
      )}
    </span>
  )
}
