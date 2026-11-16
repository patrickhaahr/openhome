import { cva } from "class-variance-authority"
import { MinusIcon } from "@phosphor-icons/react/dist/csr/Minus"
import { PlusIcon } from "@phosphor-icons/react/dist/csr/Plus"
import { Button } from "@/components/ui/button"

/** The ground-coloured veil behind a sheet that rises over the page. */
export const sheetScrim =
  "fixed inset-0 z-50 bg-scrim transition-opacity duration-fast ease-app data-ending-style:opacity-0 data-starting-style:opacity-0"

export const sheetBody = cva("grid content-start", {
  variants: { gap: { roomy: "gap-5", form: "gap-4" } },
  defaultVariants: { gap: "roomy" },
})

/** A row of keys; `submit` pins it above the sheet's bottom edge with full-width keys. */
export const sheetActions = cva("flex flex-wrap gap-2", {
  variants: {
    placement: {
      row: "",
      end: "pt-2",
      submit:
        "sticky -bottom-8 bg-plate pt-3 pb-5 **:data-[slot=button]:w-full",
    },
  },
  defaultVariants: { placement: "row" },
})

export const sheetOwner = "flex items-center gap-2 font-medium text-muted-ink"

export const sheetName =
  "text-name leading-none font-medium tracking-headline text-balance"

export const sheetNotes = cva("leading-normal whitespace-pre-line", {
  variants: {
    tone: {
      plain: "text-body-lg text-ink",
      quiet: "text-base text-muted-ink",
    },
  },
  defaultVariants: { tone: "plain" },
})

export const sheetLink =
  "inline-flex min-h-12 max-w-full items-center gap-2 justify-self-start rounded-full bg-wash-strong pr-4 pl-5 font-medium no-underline wrap-anywhere transition-colors duration-fast ease-app hover:bg-ink hover:text-ground"

export const sheetConfirm = "grid gap-3 rounded-plate bg-wash-strong px-5 py-4"

/**
 * A label above its control. The caption is the first child (a `span`, or the `legend` of a
 * fieldset); `title` makes the field's input large.
 */
export const field = cva(
  "grid min-w-0 gap-1.5 *:first:mb-0.5 *:first:text-label *:first:font-semibold *:first:text-muted-ink",
  {
    variants: {
      emphasis: {
        plain: "",
        title: "**:data-[slot=input]:h-13 **:data-[slot=input]:text-xlarge",
      },
    },
    defaultVariants: { emphasis: "plain" },
  }
)

export const fieldHint = "text-small text-muted-ink"

/** A pill that holds a leading icon and a borderless input. */
export const linkInput =
  "flex min-h-14 items-center gap-3 rounded-full bg-wash-strong px-4.5 focus-within:outline-2 focus-within:outline-offset-2 focus-within:outline-ink **:data-[slot=input]:h-14 **:data-[slot=input]:flex-1 **:data-[slot=input]:border-transparent! **:data-[slot=input]:bg-transparent **:data-[slot=input]:px-0 **:data-[slot=input]:text-large"

export const linkStatus = "-mt-3 text-small text-muted-ink empty:hidden"

export const mediaMeta = "flex items-center gap-2 text-small text-muted-ink"

export const mediaTitle =
  "line-clamp-2 text-control leading-compact font-medium"

/** The dl of a sheet's dated facts: a quiet term over its value. */
export const facts = cva("flex items-center", {
  variants: {
    flow: {
      row: "gap-7",
      wrap: "flex-wrap gap-x-7 gap-y-3",
    },
  },
  defaultVariants: { flow: "row" },
})

export const factTerm = "text-label font-semibold text-muted-ink"

export const factValue = "text-xlarge font-medium"

/** Quantity as a minus key, the figure and a plus key; `max` stops the plus key. */
export function Stepper({
  labelId,
  value,
  disabled,
  max,
  set,
}: {
  labelId: string
  value: number
  disabled: boolean
  max?: number
  set: (next: number) => void
}): React.JSX.Element {
  return (
    <div
      data-slot="stepper"
      className="flex items-center gap-2"
      role="group"
      aria-labelledby={labelId}
    >
      <Button
        type="button"
        variant="outline"
        size="key-icon"
        disabled={disabled || value <= 1}
        onClick={() => set(value - 1)}
      >
        <MinusIcon size={18} aria-hidden="true" />
        <span className="sr-only">One fewer</span>
      </Button>
      <output
        className="min-w-digits text-center text-xlarge font-medium num"
        aria-live="polite"
      >
        {value}
      </output>
      <Button
        type="button"
        variant="outline"
        size="key-icon"
        disabled={disabled || (max !== undefined && value >= max)}
        onClick={() => set(value + 1)}
      >
        <PlusIcon size={18} aria-hidden="true" />
        <span className="sr-only">One more</span>
      </Button>
    </div>
  )
}
