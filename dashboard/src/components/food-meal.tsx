import { Amount } from "@/components/food-amount"
import { useState } from "react"
import { PhotoViewer, ViewerBar, ViewerStage } from "@/components/photo-viewer"
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog"
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

const portionRow =
  "grid grid-cols-portion items-baseline gap-x-4 gap-y-2 max-sm:gap-x-3"

const portionFigure = "num text-right text-muted-ink"

export function Meal({ meal }: { readonly meal: MealLog }): React.JSX.Element {
  return (
    <article
      data-slot="food-meal"
      className="grid gap-x-food-plate gap-y-4 rounded-sheet bg-wash p-food-meal data-photo:grid-cols-trail data-photo:items-start max-sm:data-photo:grid-cols-1"
      data-photo={meal.photo === null ? undefined : ""}
    >
      <div className="grid min-w-0 content-start gap-3">
        <MealDetails meal={meal} />
      </div>
      {meal.photo === null ? null : (
        <MealPhotoKey photo={meal.photo} time={mealTime(meal.eaten_at)} />
      )}
    </article>
  )
}

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
        data-slot="food-photo"
        className="cursor-zoom-in overflow-hidden rounded-plate bg-wash-strong transition-transform duration-fast ease-app active:scale-97 max-sm:order-first"
        aria-label={`Show the photo of the meal at ${time}`}
        onClick={() => setOpen(true)}
      >
        <img
          src={mealPhotoUrl(photo, "thumbnail")}
          width={photo.thumbnail.width}
          height={photo.thumbnail.height}
          alt=""
          className="max-h-48 w-food-thumb object-cover max-sm:max-h-56 max-sm:w-full"
          loading="lazy"
          decoding="async"
        />
      </button>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent variant="fullscreen" showCloseButton={false}>
          <PhotoViewer data-slot="food-photo-viewer">
            <DialogTitle className="sr-only">Meal at {time}</DialogTitle>
            <ViewerStage>
              <img
                src={mealPhotoUrl(photo, "photo")}
                width={photo.width}
                height={photo.height}
                alt={`The meal at ${time}`}
                className="size-full object-contain"
                decoding="async"
              />
            </ViewerStage>
            <ViewerBar>Meal at {time}</ViewerBar>
          </PhotoViewer>
        </DialogContent>
      </Dialog>
    </>
  )
}

function MealDetails({ meal }: { readonly meal: MealLog }): React.JSX.Element {
  return (
    <>
      <header className="flex items-baseline justify-between gap-4">
        <h3 className="text-lead font-medium tracking-food-heading tabular-nums">
          <time dateTime={meal.eaten_at}>{mealTime(meal.eaten_at)}</time>
        </h3>
        <Amount
          total={meal.totals.energy_kcal}
          className="text-large font-semibold"
        />
      </header>
      {meal.meal_name === null ? null : (
        <p
          data-slot="food-meal-from"
          className="max-w-full justify-self-start truncate rounded-full bg-wash-strong px-2.5 py-0.5 text-label font-semibold"
        >
          <span className="sr-only">Logged from the Meal </span>
          {meal.meal_name}
        </p>
      )}
      {meal.note === null ? null : (
        <p data-slot="food-note" className="text-muted-ink">
          {meal.note}
        </p>
      )}
      <ul
        data-slot="food-portions"
        className="grid gap-1.5"
        aria-label="Portions"
      >
        {meal.portions.map((portion, index) => (
          <li key={`${portion.food_id}-${index}`} className={portionRow}>
            <PortionTitle portion={portion} />
            <span className={portionFigure}>
              {formatAmount(portion.grams)} g
            </span>
            <span className={portionFigure}>
              {portion.nutrients.energy_kcal.value === null ? (
                <span data-gap="true" className="data-[gap=true]:gap-dash">
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
        <ul
          data-slot="food-doses"
          className="grid gap-1.5"
          aria-label="Supplement Doses"
        >
          {meal.supplement_doses.map((dose) => {
            const kcal = doseEnergy(dose)

            return (
              <li key={dose.supplement_id} className={portionRow}>
                <span className="min-w-0 wrap-anywhere">{dose.name}</span>
                <span className={portionFigure}>
                  {doseCount(dose.doses, dose.dose_unit)}
                </span>
                <span className={portionFigure}>
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

function PortionTitle({
  portion,
}: {
  readonly portion: Portion
}): React.JSX.Element {
  const { name, brand } = portionName(portion)

  return (
    <span className="min-w-0 wrap-anywhere">
      {name}
      {portion.nova_class === null ? null : (
        <span
          data-slot="food-portion-nova"
          className="ms-2 inline-block rounded-full bg-wash-strong px-2 py-px align-capsule text-caption font-semibold whitespace-nowrap text-muted-ink data-[nova=1]:text-ink"
          data-nova={portion.nova_class}
          title={`NOVA ${portion.nova_class}: ${novaClassMeaning[portion.nova_class]}`}
        >
          <span className="sr-only">, </span>
          NOVA {portion.nova_class}
        </span>
      )}
      {brand === null ? null : (
        <span
          data-slot="food-portion-brand"
          className="block text-label text-muted-ink"
        >
          <span className="sr-only">, </span>
          {brand}
        </span>
      )}
    </span>
  )
}
