import { useTransition } from "react"
import { useRouter } from "@tanstack/react-router"
import { Button } from "@/components/ui/button"

/** Re-run the current page's OpenHome checks without reloading the page. */
export function CheckAgainButton(): React.JSX.Element {
  const router = useRouter()
  const [isChecking, startTransition] = useTransition()

  function checkAgain(): void {
    startTransition(async () => {
      await router.invalidate({ sync: true })
    })
  }

  return (
    <Button variant="outline" disabled={isChecking} onClick={checkAgain}>
      {isChecking ? "Checking…" : "Check again"}
    </Button>
  )
}
