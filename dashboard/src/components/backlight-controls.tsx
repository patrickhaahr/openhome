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
  type Debounce,
  type Delivery,
  discard,
  describe,
} from "@/components/backlight-delivery"
import { BacklightReadout } from "@/components/backlight-readout"
import { BacklightSetup } from "@/components/backlight-setup"
import { BacklightDock, type DockKey } from "@/components/backlight-dock"
import { lightFor, signal } from "@/components/backlight-look"

// The halo behind the TV is the light itself, thrown wide: a full-size light layer under a radial mask.
const halo =
  "pointer-events-none absolute -z-1 halo-mask transition-fade-scale duration-mid ease-app group-data-[mode=off]/stage:scale-70 group-data-[mode=off]/stage:opacity-0 group-data-[mode=off]/stage:duration-slow"

const beam = "absolute -inset-1/2 transition-colors duration-mid ease-app"

type Props = {
  data: BacklightRead
  change: (action: BacklightAction) => Promise<BacklightResult>
  /** The display clock, for the sun on the bedtime ring. */
  now?: Date
}

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

  // The delivery owner: `delivery` is the only record of what is waiting or in flight. Every
  // change to it goes through `commit`, and only `edit`, `perform`, `fire`, `deliver` and the
  // unmount cleanup below construct a variant, so a timer or response from replaced work can
  // be recognised by identity and ignored.
  const delivery = useRef<Delivery>({ phase: "idle" })
  const runs = useRef(0)
  const latestData = useRef(data)
  useEffect(() => {
    latestData.current = data
  }, [data])
  const [draft, setDraft] = useState<BacklightSelection | null>(null)
  useIdleBlock(pending)

  // `pending` and `draft` follow the delivery phase: busy while debouncing or sending, released
  // when nothing is waiting. A closed delivery belongs to an unmounted component; nothing renders.
  function commit(next: Delivery): void {
    delivery.current = next

    if (next.phase === "idle") {
      setPending(false)
      setDraft(null)
    } else if (next.phase !== "closed") setPending(true)
  }

  useEffect(
    () => () => {
      const closing = delivery.current

      if (closing.phase === "debouncing") clearTimeout(closing.debounce.timer)
      else if (closing.phase === "sending") discard(closing.next)
      // The request in flight cannot be recalled; its response finds a closed delivery and is dropped.
      delivery.current = { phase: "closed" }
    },
    []
  )

  function startDebounce(selection: BacklightSelection): Debounce {
    const debounce: Debounce = {
      kind: "debounce",
      selection,
      timer: setTimeout(() => fire(debounce), 200),
    }

    return debounce
  }

  /** The trailing 200 ms edit settled: send it now, or queue it behind the request in flight. */
  function fire(debounce: Debounce): void {
    const state = delivery.current

    const action: BacklightAction = {
      action: "select",
      selection: debounce.selection,
    }

    if (state.phase === "debouncing" && state.debounce === debounce)
      void deliver(action)
    else if (state.phase === "sending" && state.next === debounce)
      commit({ ...state, next: { kind: "queued", action } })
  }

  async function deliver(action: BacklightAction): Promise<void> {
    const run = ++runs.current
    commit({ phase: "sending", run, action, next: null })
    setSent(action.action)
    setAlert(null)
    setMessage("Sending your latest selection…")

    // Demonstrated bound: on the server, `changeBacklight` always settles. Its request carries
    // `AbortSignal.timeout(65_000)`, still attached while the body is parsed, and every failure
    // becomes an `unavailable` value. Not demonstrated: the browser's call to `updateBacklight`
    // (no abort signal is passed to the server function) and the `router.invalidate` the page
    // awaits afterwards. If either never settles, `sending` holds the interaction busy and
    // later actions queue behind it. No timeout or retry is added without a source-backed
    // requirement; see "Dashboard delivery" in docs/backlight.md.
    let result: BacklightResult | null = null

    try {
      result = await change(action)
    } catch {
      /* A rejection is reported as an unconfirmed command below. */
    }

    // Replaced or unmounted work: drop the response without touching state.
    const sending = delivery.current

    if (sending.phase !== "sending" || sending.run !== run) return

    // An older response must not replace feedback for a newer waiting edit.
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

    // A later selection must still drain; a settled edit keeps the interaction busy until sent.
    const next = sending.next

    if (next === null) commit({ phase: "idle" })
    else if (next.kind === "queued") void deliver(next.action)
    else commit({ phase: "debouncing", debounce: next })
  }

  function perform(action: BacklightAction): void {
    const state = delivery.current

    if (state.phase === "closed") return

    if (action.action === "select") setDraft(action.selection)

    if (state.phase === "sending") {
      discard(state.next)
      commit({ ...state, next: { kind: "queued", action } })

      return
    }

    if (state.phase === "debouncing") clearTimeout(state.debounce.timer)
    void deliver(action)
  }

  function edit(selection: BacklightSelection): void {
    const state = delivery.current

    if (state.phase === "closed") return
    setDraft(selection)
    setMessage("Waiting for your latest adjustment…")

    // A single trailing target, even while another request is in flight: it replaces any
    // queued action as well as an earlier edit.
    if (state.phase === "sending") {
      discard(state.next)
      commit({ ...state, next: startDebounce(selection) })

      return
    }

    if (state.phase === "debouncing") clearTimeout(state.debounce.timer)
    commit({ phase: "debouncing", debounce: startDebounce(selection) })
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
