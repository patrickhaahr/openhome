import { useEffect, useRef, useState } from "react"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { useIdleBlock } from "@/components/idle-block"
import type {
  BacklightAction,
  BacklightRead,
  BacklightResult,
  BacklightSelection,
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
  const queued = useRef<BacklightAction | null>(null)
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const mounted = useRef(true)
  const latestData = useRef(data)
  useEffect(() => {
    latestData.current = data
  }, [data])
  const [draft, setDraft] = useState<BacklightSelection | null>(null)
  useIdleBlock(pending)

  useEffect(() => {
    mounted.current = true

    return () => {
      mounted.current = false

      if (timer.current !== null) clearTimeout(timer.current)
      queued.current = null
    }
  }, [])

  async function drain(): Promise<void> {
    if (inFlight.current) return
    inFlight.current = true
    setPending(true)

    while (queued.current !== null && mounted.current) {
      const action = queued.current
      queued.current = null
      setMessage("Sending your latest selection…")

      // An older response must not replace feedback for a newer queued edit.
      const announce = (text: string): void => {
        if (queued.current === null && timer.current === null) setMessage(text)
      }

      try {
        const result = await change(action)

        if (!mounted.current) return

        if (result.state === "available") {
          setLocal({ base: latestData.current, value: result })

          if (result.status.command === "failed")
            announce(
              "OpenHome couldn’t confirm the command. Your selection is saved; check the connection and try again."
            )
          else if (action.action === "profile")
            announce("TV Sync settings saved safely.")
          else if (action.action === "refresh")
            announce(
              result.status.reachability === "reachable"
                ? "Backlight state refreshed."
                : "The Backlight couldn’t be reached."
            )
          else if (result.status.command === "pending")
            announce("Your selection is saved and waiting for the Backlight.")
          else
            announce(
              "Command acknowledged. Readable device state is shown below."
            )
        } else if (result.state === "profile-required")
          announce(
            "TV Sync settings are needed. Activate TV Sync on the controller and capture, or recover the private backup."
          )
        else if (result.state === "invalid-profile")
          announce(
            "The saved profile is invalid. Your previous TV Sync settings were kept."
          )
        else
          announce(
            "OpenHome couldn’t confirm the command. The LED may have changed; refresh before trying again."
          )
      } catch {
        if (mounted.current)
          announce(
            "OpenHome couldn’t confirm the command. The LED may have changed; refresh before trying again."
          )
      }
    }

    inFlight.current = false

    if (mounted.current && timer.current === null) {
      setPending(false)
      setDraft(null)
    }
  }

  function perform(action: BacklightAction): void {
    if (timer.current !== null) clearTimeout(timer.current)
    timer.current = null
    queued.current = action

    if (action.action === "select") setDraft(action.selection)
    void drain()
  }

  function edit(selection: BacklightSelection): void {
    setDraft(selection)
    setPending(true)
    setMessage("Waiting for your latest adjustment…")

    if (timer.current !== null) clearTimeout(timer.current)
    // A single trailing target, even while another request is in flight.
    queued.current = null
    timer.current = setTimeout(() => {
      timer.current = null
      queued.current = { action: "select", selection }
      void drain()
    }, 200)
  }

  const status = current.state === "available" ? current.status : undefined
  const observed = status?.observed
  const selection = draft ?? status?.intended

  const rgb =
    selection?.mode === "color"
      ? selection.rgb
      : { red: 255, green: 0, blue: 0 }

  const brightness =
    selection?.mode === "color" || selection?.mode === "tv-sync"
      ? (selection.brightness ?? observed?.brightness ?? 20)
      : (observed?.brightness ?? 20)

  const hex = `#${[rgb.red, rgb.green, rgb.blue].map((value) => value.toString(16).padStart(2, "0")).join("")}`

  return (
    <div className="backlight-board" aria-busy={pending}>
      <section className="backlight-main" aria-label="LED controls">
        <div className="backlight-intro">
          <p className="backlight-caption">Behind the picture</p>
          <h2>LED</h2>
          <p>
            Follow the TV picture, choose a color, or turn the Backlight off.
          </p>
        </div>
        <div
          className="backlight-modes"
          role="group"
          aria-label="Backlight selection"
        >
          <Button
            size="key-xl"
            variant="outline"
            disabled={status === undefined}
            aria-pressed={selection?.mode === "off"}
            onClick={() =>
              perform({ action: "select", selection: { mode: "off" } })
            }
          >
            Off
          </Button>
          <Button
            size="key-xl"
            variant="outline"
            disabled={status === undefined || status.setup !== "ready"}
            aria-pressed={selection?.mode === "tv-sync"}
            onClick={() =>
              perform({ action: "select", selection: { mode: "tv-sync" } })
            }
          >
            TV Sync
          </Button>
          <Button
            size="key-xl"
            variant="outline"
            disabled={status?.setup !== "ready"}
            aria-pressed={selection?.mode === "color"}
            onClick={() =>
              perform({
                action: "select",
                selection: { mode: "color", rgb, brightness },
              })
            }
          >
            Color
          </Button>
        </div>
        <section className="backlight-setup mb-8" aria-label="Bedtime Mode">
          <h3>Bedtime Mode</h3>
          <p>
            {status === undefined ? (
              "Schedule unavailable."
            ) : (
              <>
                Red at {status.bedtime.brightness}% · {status.bedtime.start}–
                {status.bedtime.end} Europe/Copenhagen. Then TV Sync, with its
                saved brightness.
              </>
            )}
          </p>
          <div className="backlight-actions">
            <Button
              variant="outline"
              role="switch"
              aria-label="Bedtime Mode enabled"
              aria-checked={status?.bedtime.enabled === true}
              disabled={pending || status?.setup !== "ready"}
              onClick={() =>
                perform({
                  action: "bedtime",
                  enabled: status?.bedtime.enabled !== true,
                })
              }
            >
              {status?.bedtime.enabled === true ? "Enabled" : "Disabled"}
            </Button>
            <Button
              variant="outline"
              disabled={status?.setup !== "ready"}
              aria-pressed={status?.control_source === "automatic"}
              onClick={() => perform({ action: "automatic" })}
            >
              Automatic
            </Button>
          </div>
        </section>
        <fieldset
          className="backlight-adjustments"
          disabled={status?.setup !== "ready"}
        >
          <legend>Manual selection</legend>
          <label className="backlight-color">
            <span>RGB color</span>
            <Input
              type="color"
              aria-label="RGB color"
              value={hex}
              onChange={(event) => {
                const value = event.currentTarget.value

                if (!/^#[0-9a-f]{6}$/i.test(value)) return
                edit({
                  mode: "color",
                  rgb: {
                    red: Number.parseInt(value.slice(1, 3), 16),
                    green: Number.parseInt(value.slice(3, 5), 16),
                    blue: Number.parseInt(value.slice(5, 7), 16),
                  },
                  brightness,
                })
              }}
            />
          </label>
          <label className="backlight-brightness">
            <span>
              Brightness <output>{brightness}%</output>
            </span>
            <Input
              type="range"
              aria-label="Brightness"
              min={0}
              max={100}
              step={1}
              value={brightness}
              disabled={selection?.mode === "off"}
              onChange={(event) => {
                const value = event.currentTarget.valueAsNumber

                if (
                  !Number.isInteger(value) ||
                  value < 0 ||
                  value > 100 ||
                  selection?.mode === "off" ||
                  selection === undefined
                )
                  return
                edit({ ...selection, brightness: value })
              }}
            />
          </label>
          <p>
            {status?.bedtime.enabled === true
              ? "Manual selections last until the next Bedtime Mode transition. Automatic resumes the routine now."
              : "Your manual selection stays until you choose again. Automatic restores TV Sync."}{" "}
            TV Sync restores its saved brightness.
          </p>
        </fieldset>
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
                  perform({ action: "profile", source: "capture" })
                }
              >
                Capture TV Sync
              </Button>
              <Button
                variant="ghost"
                disabled={pending}
                onClick={() => perform({ action: "profile", source: "backup" })}
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
            <dt>Control</dt>
            <dd>
              {status === undefined
                ? "Unknown"
                : status.control_source === "manual"
                  ? "Manual override"
                  : "Automatic"}
            </dd>
          </div>
          {status?.control_source === "manual" ? (
            <div>
              <dt>Override ends</dt>
              <dd>
                {status.override_expires_at === null ? (
                  "Until another selection"
                ) : (
                  <time dateTime={status.override_expires_at}>
                    {new Intl.DateTimeFormat("en-GB", {
                      timeZone: "Europe/Copenhagen",
                      dateStyle: "medium",
                      timeStyle: "short",
                    }).format(new Date(status.override_expires_at))}{" "}
                    Copenhagen
                  </time>
                )}
              </dd>
            </div>
          ) : null}
          <div>
            <dt>Selected</dt>
            <dd>
              {status === undefined
                ? "Unknown"
                : modeLabel[selection?.mode ?? status.intended.mode]}
            </dd>
          </div>
          {selection?.mode === "color" ? (
            <div>
              <dt>Requested RGB</dt>
              <dd>
                {selection.rgb.red}, {selection.rgb.green}, {selection.rgb.blue}
              </dd>
            </div>
          ) : null}
          {selection?.mode === "color" ||
          (selection?.mode === "tv-sync" &&
            selection.brightness !== undefined) ? (
            <div>
              <dt>Requested brightness</dt>
              <dd>{selection.brightness}%</dd>
            </div>
          ) : null}
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
            <dt>Observed brightness</dt>
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
          onClick={() => perform({ action: "refresh" })}
        >
          Refresh state
        </Button>
      </aside>
    </div>
  )
}
