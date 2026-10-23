import { useRef, useState } from "react"
import { Button } from "@/components/ui/button"
import type {
  BacklightAction,
  BacklightRead,
  BacklightResult,
} from "@/lib/backlight"

type Props = {
  data: BacklightRead
  change: (action: BacklightAction) => Promise<BacklightResult>
}

const modeLabel = {
  off: "Off",
  "tv-sync": "TV Sync",
  color: "Color",
  other: "Other",
}

const connectionLabel = {
  unconfigured: "Setup needed",
  unknown: "Not checked",
  reachable: "Reachable",
  unreachable: "Unreachable",
}

/** Commands report write acceptance separately from the controller's readable state. */
export function BacklightControls({ data, change }: Props): React.JSX.Element {
  const [local, setLocal] = useState<{
    base: BacklightRead
    value: BacklightRead
  } | null>(null)

  const current = local?.base === data ? local.value : data
  const [pending, setPending] = useState(false)

  const [message, setMessage] = useState(
    "Choose how the LED follows your evening."
  )

  const inFlight = useRef(false)

  async function perform(action: BacklightAction): Promise<void> {
    if (inFlight.current) return
    inFlight.current = true
    setPending(true)
    setMessage("Contacting the Backlight…")

    try {
      const result = await change(action)

      if (result.state === "available") {
        setLocal({ base: data, value: result })

        if (action.action === "select" && result.status.command === "failed")
          setMessage(
            "OpenHome couldn’t confirm the command. Check the connection and try again."
          )
        else if (action.action === "profile")
          setMessage("TV Sync settings saved safely.")
        else if (action.action === "refresh")
          setMessage(
            result.status.reachability === "reachable"
              ? "Backlight state refreshed."
              : "The Backlight couldn’t be reached."
          )
        else
          setMessage(
            "Command acknowledged. Readable device state is shown below."
          )
      } else if (result.state === "profile-required")
        setMessage(
          "TV Sync settings are needed. Activate TV Sync on the controller and capture, or recover the private backup."
        )
      else if (result.state === "invalid-profile")
        setMessage(
          "The saved profile is invalid. Your previous TV Sync settings were kept."
        )
      else
        setMessage(
          "OpenHome couldn’t confirm the command. The LED may have changed; refresh before trying again."
        )
    } catch {
      setMessage(
        "OpenHome couldn’t confirm the command. The LED may have changed; refresh before trying again."
      )
    } finally {
      inFlight.current = false
      setPending(false)
    }
  }

  const status = current.state === "available" ? current.status : undefined
  const observed = status?.observed

  return (
    <div className="backlight-board" aria-busy={pending}>
      <section className="backlight-main" aria-label="LED controls">
        <div className="backlight-intro">
          <p className="backlight-caption">Behind the picture</p>
          <h2>LED</h2>
          <p>
            Follow the TV picture with your saved camera settings, or turn the
            Backlight off.
          </p>
        </div>
        <div className="backlight-modes" aria-label="Backlight selection">
          <Button
            size="key-xl"
            variant="outline"
            disabled={pending || status === undefined}
            aria-pressed={status?.intended.mode === "off"}
            onClick={() =>
              void perform({ action: "select", selection: { mode: "off" } })
            }
          >
            Off
          </Button>
          <Button
            size="key-xl"
            variant="outline"
            disabled={
              pending || status === undefined || status.setup !== "ready"
            }
            aria-pressed={status?.intended.mode === "tv-sync"}
            onClick={() =>
              void perform({ action: "select", selection: { mode: "tv-sync" } })
            }
          >
            TV Sync
          </Button>
        </div>
        <p role="status" aria-live="polite" className="backlight-feedback">
          {status === undefined
            ? "OpenHome is unavailable. Refresh to try again."
            : message}
        </p>
        {status?.setup === "incomplete" ? (
          <section className="backlight-setup" aria-label="TV Sync setup">
            <h3>Save your TV Sync settings</h3>
            <p>
              Capture while TV Sync is active, or recover the installer’s
              private backup. Off stays available during setup.
            </p>
            <div className="backlight-actions">
              <Button
                variant="outline"
                disabled={pending}
                onClick={() =>
                  void perform({ action: "profile", source: "capture" })
                }
              >
                Capture TV Sync
              </Button>
              <Button
                variant="ghost"
                disabled={pending}
                onClick={() =>
                  void perform({ action: "profile", source: "backup" })
                }
              >
                Recover saved profile
              </Button>
            </div>
          </section>
        ) : null}
      </section>
      <aside className="backlight-state" aria-label="Backlight state">
        <h3>Backlight</h3>
        <dl>
          <div>
            <dt>Selected</dt>
            <dd>
              {status === undefined
                ? "Unknown"
                : modeLabel[status.intended.mode]}
            </dd>
          </div>
          <div>
            <dt>Last acknowledged</dt>
            <dd>
              {status?.acknowledged === null ||
              status?.acknowledged === undefined
                ? "None"
                : modeLabel[status.acknowledged.mode]}
            </dd>
          </div>
          <div>
            <dt>Connection</dt>
            <dd>
              {status === undefined
                ? "Unavailable"
                : connectionLabel[status.reachability]}
            </dd>
          </div>
          <div>
            <dt>Observation</dt>
            <dd>{status?.stale === false ? "Fresh" : "Stale / unknown"}</dd>
          </div>
          <div>
            <dt>Observed power</dt>
            <dd>
              {observed === null || observed === undefined
                ? "Unknown"
                : observed.power
                  ? "On"
                  : "Off"}
            </dd>
          </div>
          <div>
            <dt>Observed mode</dt>
            <dd>
              {observed === undefined || observed === null
                ? "Unknown"
                : modeLabel[observed.mode]}
            </dd>
          </div>
          <div>
            <dt>Brightness</dt>
            <dd>
              {observed === null || observed === undefined
                ? "Unknown"
                : `${observed.brightness}%`}
            </dd>
          </div>
        </dl>
        <p>
          Readback reports power, mode and brightness. It does not confirm the
          exact visible color.
        </p>
        <Button
          variant="ghost"
          disabled={pending}
          onClick={() => void perform({ action: "refresh" })}
        >
          Refresh state
        </Button>
      </aside>
    </div>
  )
}
