import { BedIcon } from "@phosphor-icons/react/dist/csr/Bed"
import { HourglassMediumIcon } from "@phosphor-icons/react/dist/csr/HourglassMedium"
import { SunIcon } from "@phosphor-icons/react/dist/csr/Sun"
import { clockDuration, spokenDuration } from "@/lib/recovery-score"
import type { RecoveryDay } from "@/lib/training"

type Session = RecoveryDay["sleep"]["sessions"][number]

type Stage = NonNullable<Session["stages"]>[number]["stage"]

/** Conventional hypnogram order: awake on top, deep at the bottom. */
const lanes = [
  "awake",
  "rem",
  "light",
  "deep",
] as const satisfies readonly Stage[]

const laneNames: Record<Stage, string> = {
  awake: "Awake",
  rem: "REM",
  light: "Light",
  deep: "Deep",
}

const hour = 3_600_000

const clock = new Intl.DateTimeFormat("en-GB", {
  timeZone: "Europe/Copenhagen",
  hour: "2-digit",
  minute: "2-digit",
  hourCycle: "h23",
})

/** Hour ticks inside the night, away from both edges so no label sits on a time above it. */
function hourTicks(from: number, to: number): readonly number[] {
  const span = to - from
  const first = Math.ceil(from / hour)
  const last = Math.floor(to / hour)

  return Array.from(
    { length: Math.max(0, last - first + 1) },
    (_, index) => (first + index) * hour
  ).filter((time) => (time - from) / span > 0.04 && (to - time) / span > 0.04)
}

/**
 * One night in time order: bedtime to wake across, a lane per stage, each stretch of the
 * session as a solid bar in its lane. The totals sit beside the lanes. The graphic is
 * decorative; a table of the segments carries it for assistive technology.
 */
export function Hypnogram({
  session,
  asleep = null,
}: {
  readonly session: Session
  /** The night's minutes asleep, shown between bedtime and wake. */
  readonly asleep?: number | null
}): React.JSX.Element {
  const from = Date.parse(session.start)
  const to = Date.parse(session.end)
  const span = to - from

  const segments = (session.stages ?? []).map((segment) => ({
    ...segment,
    begin: Date.parse(segment.start),
    finish: Date.parse(segment.end),
  }))

  const minutes = (stage: Stage): number =>
    segments
      .filter((segment) => segment.stage === stage)
      .reduce(
        (sum, segment) => sum + (segment.finish - segment.begin) / 60_000,
        0
      )

  const ticks = hourTicks(from, to)

  // Alternate hours are a darker ground, so the hours read without a line between them.
  const bands = ticks.filter((_, index) => index % 2 === 0)

  return (
    <figure className="hypno">
      <p className="hypno-times" aria-hidden="true">
        <span>
          <BedIcon size={18} />
          {clock.format(from)}
        </span>
        {asleep === null ? null : (
          <span>
            <HourglassMediumIcon size={18} />
            {clockDuration(asleep)}
          </span>
        )}
        <span>
          <SunIcon size={18} />
          {clock.format(to)}
        </span>
      </p>
      <div className="hypno-labels" aria-hidden="true">
        {lanes.map((stage) => (
          <p key={stage} data-stage={stage}>
            {laneNames[stage]}
            <span className="num">{clockDuration(minutes(stage))}</span>
          </p>
        ))}
      </div>
      <div className="hypno-plot" aria-hidden="true">
        {bands.map((time) => (
          <i
            key={time}
            className="hypno-band"
            // SAFETY: CSSProperties omits custom properties; both are 0..1 shares of the night.
            style={
              {
                "--from": (time - from) / span,
                "--length": Math.min(hour, to - time) / span,
              } as React.CSSProperties
            }
          />
        ))}
        {lanes.map((stage) => (
          <div key={stage} className="hypno-lane" data-stage={stage}>
            {segments
              .filter((segment) => segment.stage === stage)
              .map((segment) => (
                <span
                  key={segment.start}
                  // SAFETY: CSSProperties omits custom properties; both are 0..1 shares of the night.
                  style={
                    {
                      "--from": (segment.begin - from) / span,
                      "--length": (segment.finish - segment.begin) / span,
                    } as React.CSSProperties
                  }
                />
              ))}
          </div>
        ))}
      </div>
      <p className="hypno-axis" aria-hidden="true">
        {ticks.map((time) => (
          <span
            key={time}
            // SAFETY: CSSProperties omits custom properties; --from is a 0..1 share of the night.
            style={{ "--from": (time - from) / span } as React.CSSProperties}
          >
            {clock.format(time)}
          </span>
        ))}
      </p>
      <table className="sr-only">
        <caption>
          Sleep stages in order, asleep {clock.format(from)}, awake{" "}
          {clock.format(to)}
          {asleep === null ? "" : `, ${spokenDuration(asleep)} slept`}
        </caption>
        <thead>
          <tr>
            <th scope="col">Stage</th>
            <th scope="col">From</th>
            <th scope="col">To</th>
            <th scope="col">Length</th>
          </tr>
        </thead>
        <tbody>
          {segments.map((segment) => (
            <tr key={segment.start}>
              <th scope="row">{laneNames[segment.stage]}</th>
              <td>{clock.format(segment.begin)}</td>
              <td>{clock.format(segment.finish)}</td>
              <td>
                {spokenDuration((segment.finish - segment.begin) / 60_000)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </figure>
  )
}
