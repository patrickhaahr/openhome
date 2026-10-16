import { Link } from "@tanstack/react-router"
import { PlugsIcon } from "@phosphor-icons/react/dist/csr/Plugs"
import { WarningCircleIcon } from "@phosphor-icons/react/dist/csr/WarningCircle"

/**
 * A failure drawn first and worded second: a warning mark and one or two words. Hidden from
 * assistive technology because the control's live region already says it in full.
 */
export function ControlAlert({ tag }: { tag: string }): React.JSX.Element {
  return (
    <span className="control-alert" aria-hidden="true">
      <WarningCircleIcon size={18} weight="fill" />
      {tag}
    </span>
  )
}

/** A round key to the Connection page, named for assistive technology. */
export function OpenConnection(): React.JSX.Element {
  return (
    <Link to="/connection" className="key-link" title="Open Connection">
      <PlugsIcon size={20} aria-hidden="true" />
      <span className="sr-only">Open Connection</span>
    </Link>
  )
}
