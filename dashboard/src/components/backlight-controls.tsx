import { useEffect, useRef, useState } from "react"
import { ApertureIcon } from "@phosphor-icons/react/dist/csr/Aperture"
import { ArrowsClockwiseIcon } from "@phosphor-icons/react/dist/csr/ArrowsClockwise"
import { ClockCounterClockwiseIcon } from "@phosphor-icons/react/dist/csr/ClockCounterClockwise"
import { ArrowCounterClockwiseIcon } from "@phosphor-icons/react/dist/csr/ArrowCounterClockwise"
import { ControlAlert } from "@/components/control-alert"
import { BedtimeDial } from "@/components/bedtime-dial"
import { HueWheel } from "@/components/hue-wheel"
import { LightFader } from "@/components/light-fader"
import { Button } from "@/components/ui/button"
import { useIdleBlock } from "@/components/idle-block"
import { copenhagenClockTime, defaultRgb, rgbCss } from "@/lib/backlight-light"
import type {
  BacklightAction,
  BacklightRead,
  BacklightResult,
  BacklightSelection,
} from "@/lib/backlight"

type Props = {
  data: BacklightRead
  change: (action: BacklightAction) => Promise<BacklightResult>
  /** The display clock, for the sun on the bedtime ring. */
  now?: Date
}

const modeLabel = {
  off: "Off",
  "tv-sync": "TV Sync",
  color: "Color",
  effect: "Effect",
  other: "Other",
}

const commandLabel = {
  idle: "No command",
  pending: "Pending",
  acknowledged: "Acknowledged",
  failed: "Failed; selection saved",
}

const recoveryLabel = {
  idle: "No recovery needed",
  pending: "Pending",
  retrying: "Retrying latest selection",
  recovered: "Current selection recovered",
  "profile-required": "TV Sync settings needed",
}

const connectionLabel = {
  unconfigured: "Setup needed",
  unknown: "Not checked",
  reachable: "Reachable",
  unreachable: "Unreachable",
}

const unconfirmed =
  "OpenHome couldn’t confirm the command. The LED may have changed; refresh before trying again."

/**
 * The LED drawn as itself: a TV on a night wall, and behind it the light it is asked to make.
 * The halo is the selection; the dock below picks one, the fader sets how far it reaches, the
 * wheel paints it and the dial schedules the bedtime red. Words live only in the live region.
 */
export function BacklightControls({
  data,
  change,
  now,
}: Props): React.JSX.Element {
  const [local, setLocal] = useState<{
    base: BacklightRead
    value: BacklightRead
  } | null>(null)

  const current = local?.base === data ? local.value : data
  const [pending, setPending] = useState(false)
  const [message, setMessage] = useState("")
  const [alert, setAlert] = useState<string | null>(null)
  const [sent, setSent] = useState<BacklightAction["action"] | null>(null)
  const [bedtimeRound, setBedtimeRound] = useState(0)

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
      setSent(action.action)
      setAlert(null)
      setMessage("Sending your latest selection…")

      // An older response must not replace feedback for a newer queued edit.
      const announce = (text: string, tag: string | null = null): void => {
        if (queued.current === null && timer.current === null) {
          setMessage(text)
          setAlert(tag)
        }
      }

      try {
        const result = await change(action)

        if (!mounted.current) return

        if (action.action === "bedtime") setBedtimeRound((round) => round + 1)

        if (result.state === "available") {
          setLocal({ base: latestData.current, value: result })

          if (result.status.command === "failed")
            announce(
              "OpenHome couldn’t confirm the command. Your selection is saved; OpenHome retries the latest selection automatically.",
              "Retrying"
            )
          else if (action.action === "profile")
            announce("TV Sync settings saved safely.")
          else if (action.action === "bedtime")
            announce(
              "Bedtime settings saved. Readable device state is listed with the Backlight state."
            )
          else if (action.action === "refresh")
            announce(
              result.status.reachability === "reachable"
                ? "Backlight state refreshed."
                : "The Backlight couldn’t be reached.",
              result.status.reachability === "reachable" ? null : "No answer"
            )
          else if (result.status.command === "pending")
            announce("Your selection is saved and waiting for the Backlight.")
          else
            announce(
              "Command acknowledged. Readable device state is listed with the Backlight state."
            )
        } else if (result.state === "profile-required")
          announce(
            "TV Sync settings are needed. Activate TV Sync on the controller and capture, or recover the private backup.",
            "Setup needed"
          )
        else if (result.state === "invalid-profile")
          announce(
            "The saved profile is invalid. Your previous TV Sync settings were kept.",
            "Not saved"
          )
        else if (result.state === "invalid-settings")
          announce(
            "Bedtime settings weren’t saved. Check the times and red brightness, then try again.",
            "Not saved"
          )
        else announce(unconfirmed, "No answer")
      } catch {
        if (mounted.current) announce(unconfirmed, "No answer")
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
  const ready = status?.setup === "ready"

  // The wheel remembers the last chosen colour while another mode is showing.
  const [lastRgb, setLastRgb] = useState(defaultRgb)
  const rgb = selection?.mode === "color" ? selection.rgb : lastRgb

  if (selection?.mode === "color" && selection.rgb !== lastRgb)
    setLastRgb(selection.rgb)

  const brightness =
    selection?.mode === "color" ||
    selection?.mode === "tv-sync" ||
    selection?.mode === "effect"
      ? (selection.brightness ?? observed?.brightness ?? 20)
      : (observed?.brightness ?? 20)

  const mode = selection?.mode ?? "off"
  const effect = selection?.mode === "effect" ? selection.effect : undefined

  const settling =
    pending ||
    status?.command === "pending" ||
    status?.recovery === "pending" ||
    status?.recovery === "retrying"

  const outcome: "pending" | "failed" | "accepted" | undefined =
    alert !== null && sent === "select"
      ? "failed"
      : status?.command === "failed"
        ? "failed"
        : settling
          ? "pending"
          : sent === "select" && status?.command === "acknowledged"
            ? "accepted"
            : undefined

  const tag =
    status === undefined
      ? "Offline"
      : (alert ?? (status.command === "failed" ? "Retrying" : null))

  const keys: {
    id: string
    label: string
    detail?: string
    selected: boolean
    disabled: boolean
    selection: BacklightSelection
  }[] = [
    {
      id: "off",
      label: "Off",
      selected: mode === "off",
      disabled: status === undefined,
      selection: { mode: "off" },
    },
    {
      id: "tv-sync",
      label: "TV Sync",
      selected: mode === "tv-sync",
      disabled: !ready,
      selection: { mode: "tv-sync" },
    },
    {
      id: "color",
      label: "Color",
      selected: mode === "color",
      disabled: !ready,
      selection: { mode: "color", rgb, brightness },
    },
    ...(status?.effects ?? []).map((item) => ({
      id: item.id,
      label: item.name,
      detail: item.description,
      selected: effect === item.id,
      disabled: !ready,
      selection: {
        mode: "effect",
        effect: item.id,
        brightness,
      } satisfies BacklightSelection,
    })),
  ]

  const override =
    status?.control_source === "manual" ? status.override_expires_at : undefined

  return (
    <div className="led" aria-busy={pending}>
      <section
        className="led-stage"
        aria-label="LED controls"
        data-mode={mode}
        data-effect={effect}
        data-ready={ready}
        // SAFETY: CSSProperties omits custom properties; --led is a CSS colour, --level 0..1.
        style={
          {
            "--led": rgbCss(rgb),
            "--level": mode === "off" ? 0 : brightness / 100,
          } as React.CSSProperties
        }
      >
        <div className="led-marks">
          <span
            className="led-signal"
            data-reach={status?.reachability ?? "unreachable"}
            title={
              status === undefined
                ? "Unavailable"
                : connectionLabel[status.reachability]
            }
            aria-hidden="true"
          />
          {status?.stale === true ? (
            <ClockCounterClockwiseIcon
              className="led-stale"
              size={18}
              weight="bold"
              aria-hidden="true"
            />
          ) : null}
          {tag === null ? null : <ControlAlert tag={tag} />}
        </div>

        <div className="led-tools">
          {override === undefined ? null : (
            <>
              {override === null ? null : (
                <span className="led-until num" aria-hidden="true">
                  {copenhagenClockTime(new Date(override))}
                </span>
              )}
              <Button
                size="key-icon"
                variant="ghost"
                disabled={!ready}
                title="Automatic"
                onClick={() => perform({ action: "automatic" })}
              >
                <ArrowCounterClockwiseIcon
                  size={22}
                  weight="bold"
                  aria-hidden="true"
                />
                <span className="sr-only">Automatic</span>
              </Button>
            </>
          )}
          <Button
            size="key-icon"
            variant="ghost"
            disabled={pending}
            data-spinning={pending && sent === "refresh"}
            title="Refresh state"
            onClick={() => perform({ action: "refresh" })}
          >
            <ArrowsClockwiseIcon size={22} weight="bold" aria-hidden="true" />
            <span className="sr-only">Refresh state</span>
          </Button>
        </div>

        <div className="led-tv">
          <div className="led-halo led-spill" aria-hidden="true">
            <span className="led-light" />
          </div>
          <div
            key={`${mode}-${effect ?? ""}`}
            className="led-halo"
            aria-hidden="true"
          >
            <span className="led-light" />
          </div>
          <div className="led-screen">
            {mode === "tv-sync" && ready ? (
              <span className="led-picture" aria-hidden="true" />
            ) : null}
            {status?.setup === "incomplete" ? (
              <section className="led-setup" aria-labelledby="led-setup">
                <h2 id="led-setup" className="sr-only">
                  Save your TV Sync settings
                </h2>
                <p className="sr-only">
                  Capture while TV Sync is active, or recover the installer’s
                  private backup. Off stays available during setup.
                </p>
                <Button
                  size="key"
                  variant="outline"
                  disabled={pending}
                  title="Capture TV Sync"
                  onClick={() =>
                    perform({ action: "profile", source: "capture" })
                  }
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
                  onClick={() =>
                    perform({ action: "profile", source: "backup" })
                  }
                >
                  <ClockCounterClockwiseIcon size={22} aria-hidden="true" />
                  <span aria-hidden="true">Restore</span>
                  <span className="sr-only">Recover saved profile</span>
                </Button>
              </section>
            ) : null}
          </div>
        </div>

        <div className="led-level">
          <LightFader
            label="Brightness"
            orientation="vertical"
            tone="var(--light, var(--led))"
            value={brightness}
            disabled={!ready || mode === "off" || selection === undefined}
            readback={
              observed === null || observed === undefined
                ? null
                : observed.brightness
            }
            onChange={(value) => {
              if (selection === undefined || selection.mode === "off") return
              edit({ ...selection, brightness: value })
            }}
          />
        </div>

        <div className="led-dock" role="group" aria-label="Backlight selection">
          {keys.map((key) => (
            <Button
              key={key.id}
              variant="ghost"
              data-key={key.id}
              disabled={key.disabled}
              aria-pressed={key.selected}
              data-outcome={key.selected ? outcome : undefined}
              title={key.label}
              // SAFETY: CSSProperties omits custom properties; --swatch is a CSS colour.
              style={
                key.id === "color"
                  ? ({ "--swatch": rgbCss(rgb) } as React.CSSProperties)
                  : undefined
              }
              onClick={() =>
                perform({ action: "select", selection: key.selection })
              }
            >
              <span className="led-swatch" aria-hidden="true" />
              <span className="sr-only">{key.label}</span>
              {key.detail === undefined ? null : (
                <span className="sr-only">{key.detail}</span>
              )}
            </Button>
          ))}
        </div>
      </section>

      <aside className="led-side" aria-label="Color and Bedtime Mode">
        <HueWheel
          rgb={rgb}
          active={mode === "color"}
          disabled={!ready}
          onChange={(next) => {
            setLastRgb(next)
            edit({ mode: "color", rgb: next, brightness })
          }}
        />
        {status === undefined ? null : (
          <BedtimeDial
            key={`${status.bedtime.start}-${status.bedtime.end}-${status.bedtime.brightness}-${bedtimeRound}`}
            saved={status.bedtime}
            now={now}
            canSwitch={ready}
            pending={pending}
            save={perform}
          />
        )}
      </aside>

      <div className="sr-only">
        <p role="status" aria-live="polite">
          {status === undefined
            ? "OpenHome is unavailable. Refresh to try again."
            : message}
        </p>
        <h2>Backlight state</h2>
        <dl>
          <dt>Control</dt>
          <dd>
            {status === undefined
              ? "Unknown"
              : status.control_source === "manual"
                ? "Manual override"
                : "Automatic"}
          </dd>
          {status?.control_source === "manual" ? (
            <>
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
            </>
          ) : null}
          <dt>Selected</dt>
          <dd>{status === undefined ? "Unknown" : modeLabel[mode]}</dd>
          {selection?.mode === "effect" ? (
            <>
              <dt>Requested effect</dt>
              <dd>
                {status?.effects.find((item) => item.id === selection.effect)
                  ?.name ?? selection.effect}
              </dd>
            </>
          ) : null}
          {selection?.mode === "color" ? (
            <>
              <dt>Requested RGB</dt>
              <dd>
                {selection.rgb.red}, {selection.rgb.green}, {selection.rgb.blue}
              </dd>
            </>
          ) : null}
          {selection?.mode === "color" ||
          selection?.mode === "effect" ||
          (selection?.mode === "tv-sync" &&
            selection.brightness !== undefined) ? (
            <>
              <dt>Requested brightness</dt>
              <dd>{selection.brightness}%</dd>
            </>
          ) : null}
          <dt>Last acknowledged</dt>
          <dd>
            {status?.acknowledged === null || status?.acknowledged === undefined
              ? "None"
              : modeLabel[status.acknowledged.mode]}
          </dd>
          <dt>Connection</dt>
          <dd>
            {status === undefined
              ? "Unavailable"
              : connectionLabel[status.reachability]}
          </dd>
          <dt>Command</dt>
          <dd>
            {status === undefined ? "Unknown" : commandLabel[status.command]}
          </dd>
          <dt>Recovery</dt>
          <dd>
            {status === undefined ? "Unknown" : recoveryLabel[status.recovery]}
          </dd>
          <dt>Observation</dt>
          <dd>{status?.stale === false ? "Fresh" : "Stale / unknown"}</dd>
          <dt>Observed power</dt>
          <dd>
            {observed === null || observed === undefined
              ? "Unknown"
              : observed.power
                ? "On"
                : "Off"}
          </dd>
          <dt>Observed mode</dt>
          <dd>
            {observed === undefined || observed === null
              ? "Unknown"
              : modeLabel[observed.mode]}
          </dd>
          <dt>Observed brightness</dt>
          <dd>
            {observed === null || observed === undefined
              ? "Unknown"
              : `${observed.brightness}%`}
          </dd>
        </dl>
        <p>
          Readback reports power, mode and brightness. It does not confirm the
          exact visible color or effect. The notch on the brightness fader marks
          the read brightness.
        </p>
        {status === undefined ? null : (
          <p>
            Bedtime Mode: red at {status.bedtime.brightness}% ·{" "}
            {status.bedtime.start}–{status.bedtime.end} Europe/Copenhagen, then
            TV Sync with its saved brightness.{" "}
            {status.bedtime.enabled === true
              ? "Manual selections last until the next Bedtime Mode transition. Automatic resumes the routine now."
              : "Your manual selection stays until you choose again. Automatic restores TV Sync."}{" "}
            TV Sync restores its saved brightness. Times follow Copenhagen,
            including summer and winter time. Settings are kept while disabled.
          </p>
        )}
      </div>
    </div>
  )
}
