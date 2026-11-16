import type { BacklightRead, BacklightSelection } from "@/lib/backlight"
import {
  commandLabel,
  connectionLabel,
  modeLabel,
  recoveryLabel,
} from "@/components/backlight-delivery"

type Status = Extract<BacklightRead, { state: "available" }>["status"]

/** The screen-reader record of the Backlight: live message, state list and the readback caveats. */
export function BacklightReadout({
  status,
  observed,
  selection,
  mode,
  message,
}: {
  readonly status: Status | undefined
  readonly observed: Status["observed"] | undefined
  readonly selection: BacklightSelection | undefined
  readonly mode: keyof typeof modeLabel
  readonly message: string
}): React.JSX.Element {
  return (
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
          {status.bedtime.start}–{status.bedtime.end} Europe/Copenhagen, then TV
          Sync with its saved brightness.{" "}
          {status.bedtime.enabled === true
            ? "Manual selections last until the next Bedtime Mode transition. Automatic resumes the routine now."
            : "Your manual selection stays until you choose again. Automatic restores TV Sync."}{" "}
          TV Sync restores its saved brightness. Times follow Copenhagen,
          including summer and winter time. Settings are kept while disabled.
        </p>
      )}
    </div>
  )
}
