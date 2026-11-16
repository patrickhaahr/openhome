import { ApertureIcon } from "@phosphor-icons/react/dist/csr/Aperture"
import { ClockCounterClockwiseIcon } from "@phosphor-icons/react/dist/csr/ClockCounterClockwise"
import { Button } from "@/components/ui/button"
import type { BacklightAction } from "@/lib/backlight"

/** The way out of an incomplete setup: capture TV Sync, or recover the installer's backup. */
export function BacklightSetup({
  pending,
  perform,
}: {
  readonly pending: boolean
  readonly perform: (action: BacklightAction) => void
}): React.JSX.Element {
  return (
    <section
      className="relative flex flex-wrap justify-center gap-2 p-4 **:data-[slot=button]:bg-led-wash-strong **:data-[slot=button]:bg-clip-border **:data-[slot=button]:text-led-ink **:data-[slot=button]:hover:bg-led-ink **:data-[slot=button]:hover:text-led-wall"
      aria-labelledby="led-setup"
    >
      <h2 id="led-setup" className="sr-only">
        Save your TV Sync settings
      </h2>
      <p className="sr-only">
        Capture while TV Sync is active, or recover the installer’s private
        backup. Off stays available during setup.
      </p>
      <Button
        size="key"
        variant="outline"
        disabled={pending}
        title="Capture TV Sync"
        onClick={() => perform({ action: "profile", source: "capture" })}
      >
        <ApertureIcon size={22} aria-hidden="true" />
        <span aria-hidden="true">Capture</span>
        <span className="sr-only">Capture TV Sync</span>
      </Button>
      <Button
        size="key"
        variant="ghost"
        disabled={pending}
        title="Recover saved profile"
        onClick={() => perform({ action: "profile", source: "backup" })}
      >
        <ClockCounterClockwiseIcon size={22} aria-hidden="true" />
        <span aria-hidden="true">Restore</span>
        <span className="sr-only">Recover saved profile</span>
      </Button>
    </section>
  )
}
