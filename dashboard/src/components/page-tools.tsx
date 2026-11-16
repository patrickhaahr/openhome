/** The page's heading when the page itself is a message: the one visible title, and the focus target. */
export function PageTitle({
  children,
}: {
  children: React.ReactNode
}): React.JSX.Element {
  return (
    <h1
      id="page-heading"
      tabIndex={-1}
      className="-ml-title-hang text-display leading-page-title font-medium tracking-page-title"
    >
      {children}
    </h1>
  )
}

/** A link that names where it goes; its arrow glyph nudges up and right on hover. */
export const actionLink =
  "inline-flex min-h-11 items-center gap-2 font-semibold text-ink hover:underline hover:decoration-2 hover:underline-offset-4 [&_svg]:transition-transform [&_svg]:duration-mid [&_svg]:ease-app hover:[&_svg]:translate-x-1 hover:[&_svg]:-translate-y-1"

/** The message for a page that has nothing to show: words, no box. The body is its paragraphs. */
export function StateNote({
  title,
  icon,
  children,
  ...rest
}: Omit<React.ComponentProps<"div">, "title"> & {
  title: string
  icon?: React.ReactNode
}): React.JSX.Element {
  return (
    <div
      className="grid max-w-136 justify-items-start gap-3 py-note-block"
      {...rest}
    >
      {icon}
      <h2 className="text-title leading-none font-medium tracking-headline">
        {title}
      </h2>
      {children}
    </div>
  )
}

/** A paragraph of a state note. */
export const stateNoteText = "text-body-lg text-muted-ink"
