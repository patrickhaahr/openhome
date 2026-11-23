import { useEffect, useRef, useState } from "react"
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
import {
  connectionLabel,
  unconfirmed,
  type Delivery,
  type DeliveryEvent,
  describe,
  initialDelivery,
  next,
} from "@/components/backlight-delivery"
import { BacklightReadout } from "@/components/backlight-readout"
import { BacklightSetup } from "@/components/backlight-setup"
import { BacklightDock, type DockKey } from "@/components/backlight-dock"
import { lightFor, signal } from "@/components/backlight-look"

const halo =
  "pointer-events-none absolute -z-1 halo-mask transition-fade-scale duration-mid ease-app group-data-[mode=off]/stage:scale-70 group-data-[mode=off]/stage:opacity-0 group-data-[mode=off]/stage:duration-slow"

const beam = "absolute -inset-1/2 transition-colors duration-mid ease-app"

type Props = {
  data: BacklightRead
  change: (action: BacklightAction) => Promise<BacklightResult>
  now?: Date
}

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

  const delivery = useRef<Delivery>(initialDelivery)
  const runs = useRef(0)
  const timers = useRef(new Map<number, ReturnType<typeof setTimeout>>())
  const latestData = useRef(data)
  useEffect(() => {
    latestData.current = data
  }, [data])
  const [draft, setDraft] = useState<BacklightSelection | null>(null)
  useIdleBlock(pending)

  function dispatch(event: DeliveryEvent): void {
    const { delivery: following, effects } = next(delivery.current, event)
    delivery.current = following

    if (following.phase === "idle") {
      setPending(false)
      setDraft(null)
    } else if (following.phase !== "closed") setPending(true)

    for (const effect of effects) {
      if (effect.kind === "cancel") cancelTimer(effect.id)
      else if (effect.kind === "schedule") scheduleTimer(effect.id)
      else void deliver(effect.action, effect.run)
    }
  }

  function cancelTimer(id: number): void {
    clearTimeout(timers.current.get(id))
    timers.current.delete(id)
  }

  function scheduleTimer(id: number): void {
    timers.current.set(
      id,
      setTimeout(() => {
        timers.current.delete(id)
        dispatch({ type: "fire", id, run: ++runs.current })
      }, 200)
    )
  }

  useEffect(
    () => () => {
      const closing = next(delivery.current, { type: "close" })
      delivery.current = closing.delivery

      for (const effect of closing.effects)
        if (effect.kind === "cancel") cancelTimer(effect.id)
    },
    []
  )

  async function deliver(action: BacklightAction, run: number): Promise<void> {
    setSent(action.action)
    setAlert(null)
    setMessage("Sending your latest selection…")

    let result: BacklightResult | null = null

    try {
      result = await change(action)
    } catch {}

    const sending = delivery.current

    if (sending.phase !== "sending" || sending.run !== run) return

    const announce = (text: string, tag: string | null = null): void => {
      if (sending.next === null) {
        setMessage(text)
        setAlert(tag)
      }
    }

    if (result === null) announce(unconfirmed, "No answer")
    else {
      if (action.action === "bedtime") setBedtimeRound((round) => round + 1)

      if (result.state === "available")
        setLocal({ base: latestData.current, value: result })

      const outcome = describe(action, result)
      announce(outcome.text, outcome.tag)
    }

    dispatch({ type: "settle", run: ++runs.current })
  }

  function perform(action: BacklightAction): void {
    if (delivery.current.phase === "closed") return

    if (action.action === "select") setDraft(action.selection)
    dispatch({ type: "perform", action, run: ++runs.current })
  }

  function edit(selection: BacklightSelection): void {
    if (delivery.current.phase === "closed") return
    setDraft(selection)
    setMessage("Waiting for your latest adjustment…")
    dispatch({ type: "edit", selection, id: ++runs.current })
  }

  const status = current.state === "available" ? current.status : undefined
  const observed = status?.observed
  const selection = draft ?? status?.intended
  const ready = status?.setup === "ready"

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

  const keys: DockKey[] = [
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

  const light = lightFor(mode, effect)

  return (
    <div
      className="grid h-led-screen grid-cols-led gap-led led-stacked:h-auto led-stacked:grid-cols-1 led-stacked:grid-rows-pair"
      aria-busy={pending}
    >
      <section
        className="group/stage @container-size relative isolate min-h-0 min-w-0 overflow-hidden rounded-4xl bg-led-wall text-led-ink led-stacked:h-led-stage led-phone:h-120 led-phone:rounded-sheet"
        aria-label="LED controls"
        data-mode={mode}
        // SAFETY: CSSProperties omits custom properties; --led is a CSS colour, --level 0..1.
        style={
          {
            "--led": rgbCss(rgb),
            "--level": mode === "off" ? 0 : brightness / 100,
          } as React.CSSProperties
        }
      >
        <div className="absolute top-flow left-flow z-3 ml-3 flex min-h-12 items-center gap-3">
          <span
            className={`size-2.5 rounded-full ${signal[status?.reachability ?? "unreachable"]}`}
            title={
              status === undefined
                ? "Unavailable"
                : connectionLabel[status.reachability]
            }
            aria-hidden="true"
          />
          {status?.stale === true ? (
            <ClockCounterClockwiseIcon
              className="text-led-muted"
              size={18}
              weight="bold"
              aria-hidden="true"
            />
          ) : null}
          {tag === null ? null : (
            <ControlAlert tag={tag} className="text-led-danger" />
          )}
        </div>

        <div className="absolute top-flow right-flow z-3 flex min-h-12 items-center gap-1 **:data-[slot=button]:text-led-muted **:data-[slot=button]:hover:bg-led-wash-strong **:data-[slot=button]:hover:bg-clip-border **:data-[slot=button]:hover:text-led-ink">
          {override === undefined ? null : (
            <>
              {override === null ? null : (
                <span
                  className="mr-1 text-body-lg font-medium num text-led-muted"
                  aria-hidden="true"
                >
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

        <div className="absolute top-11/25 left-47/100 aspect-video w-led-tv -translate-1/2 led-phone:top-9/25 led-phone:left-21/50 led-phone:w-led-tv-phone">
          <div
            className={`${halo} -inset-x-19/20 -inset-y-3/2 spill-glow`}
            aria-hidden="true"
          >
            <span className={`${beam} ${light.fill} ${light.motion}`} />
          </div>
          <div
            key={`${mode}-${effect ?? ""}`}
            className={`${halo} -inset-x-9/25 -inset-y-31/50 halo-glow motion-safe:animate-led-bloom`}
            aria-hidden="true"
          >
            <span className={`${beam} ${light.fill} ${light.motion}`} />
          </div>
          <div className="absolute inset-0 grid place-items-center overflow-hidden rounded-sm border border-led-edge bg-black transition-colors duration-mid ease-app group-data-[mode=color]/stage:screen-glow">
            {mode === "tv-sync" && ready ? (
              <span
                className="absolute inset-0 bg-led-picture before:absolute before:top-led-horizon before:left-1/2 before:aspect-square before:w-17/100 before:-translate-1/2 before:rounded-full before:bg-sun before:half-sun after:absolute after:inset-x-0 after:top-led-horizon after:h-px after:bg-led-horizon motion-safe:before:animate-led-set"
                aria-hidden="true"
              />
            ) : null}
            {status?.setup === "incomplete" ? (
              <BacklightSetup pending={pending} perform={perform} />
            ) : null}
          </div>
        </div>

        <div className="absolute top-11/25 right-flow z-2 mr-2 h-led-level w-18 -translate-y-1/2 led-phone:top-9/25 led-phone:h-led-level-phone led-phone:w-14">
          <LightFader
            label="Brightness"
            orientation="vertical"
            tone={light.fill}
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

        <BacklightDock
          keys={keys}
          rgb={rgb}
          outcome={outcome}
          perform={perform}
        />
      </section>

      <aside
        className="grid min-h-0 grid-rows-led-side items-center justify-items-center gap-led-side led-stacked:grid-cols-2 led-stacked:grid-rows-none led-phone:grid-cols-1 led-phone:gap-10 led-phone:pt-4 led-phone:pb-8"
        aria-label="Color and Bedtime Mode"
      >
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

      <BacklightReadout
        status={status}
        observed={observed}
        selection={selection}
        mode={mode}
        message={message}
      />
    </div>
  )
}
