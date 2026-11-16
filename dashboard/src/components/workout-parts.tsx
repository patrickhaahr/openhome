/**
 * A view's progress and failure line. Progress and success are acknowledged by the controls
 * themselves, so those words stay for assistive technology; a failure stays visible.
 */
export function WorkoutFeedback({
  failed,
  role = "status",
  children,
}: {
  readonly failed: boolean | undefined
  readonly role?: "status" | "alert"
  readonly children: string | undefined
}): React.JSX.Element {
  const look =
    children === undefined || children === ""
      ? "hidden"
      : failed === true
        ? "mt-2 mb-4 flex items-center gap-2 text-control font-semibold text-danger before:size-2.5 before:shrink-0 before:rounded-full before:bg-danger"
        : "sr-only"

  return (
    <p className={look} role={role} data-failed={failed}>
      {children}
    </p>
  )
}

/** A read that failed, in place of what it would have drawn: its name and why. */
export function FailurePlate({
  title,
  className = "",
  children,
}: {
  readonly title: string
  readonly className?: string
  readonly children: React.ReactNode
}): React.JSX.Element {
  return (
    <div className={`grid gap-2 rounded-sheet bg-tile p-6 ${className}`}>
      <h2 className="text-xlarge font-semibold">{title}</h2>
      {children}
    </div>
  )
}

/** A muted line under an exercise or set: why nothing is drawn, or the lifter's note. */
export const workoutMuted = "text-small leading-relaxed text-muted-ink"

/** A view's toolbar: the view switch first, the view's own tools at the other end. */
export function ViewBar({
  lead,
  children,
}: {
  readonly lead: React.ReactNode
  readonly children?: React.ReactNode
}): React.JSX.Element {
  return (
    <div className="mb-view-bar flex flex-wrap items-center justify-between gap-x-4 gap-y-3 max-[700px]:w-full">
      {lead}
      {children === undefined ? null : (
        <div className="ml-auto flex flex-wrap items-center justify-end gap-2 max-[700px]:w-full">
          {children}
        </div>
      )}
    </div>
  )
}
