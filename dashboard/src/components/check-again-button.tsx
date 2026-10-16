import { useTransition } from "react"
import { useRouter } from "@tanstack/react-router"
import { ArrowClockwiseIcon } from "@phosphor-icons/react/dist/csr/ArrowClockwise"
import { Button } from "@/components/ui/button"

/**
 * Re-run the current page's OpenHome checks without reloading the page. `compact` draws a
 * round arrow key; its name stays the button's text for assistive technology.
 */
export function CheckAgainButton({
  compact = false,
}: {
  compact?: boolean
}): React.JSX.Element {
  const router = useRouter()
  const [isChecking, startTransition] = useTransition()

  function checkAgain(): void {
    startTransition(async () => {
      await router.invalidate({ sync: true })
    })
  }

  const label = isChecking ? "Checking…" : "Check again"

  return (
    <Button
      size={compact ? "key-icon" : "lg"}
      variant="outline"
      disabled={isChecking}
      data-spinning={isChecking}
      title={compact ? label : undefined}
      onClick={checkAgain}
    >
      {compact ? (
        <>
          <ArrowClockwiseIcon size={20} aria-hidden="true" />
          <span className="sr-only">{label}</span>
        </>
      ) : (
        label
      )}
    </Button>
  )
}
