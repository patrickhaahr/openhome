/**
 * The page's one heading, for assistive technology and focus after navigation. The top bar
 * already marks where you are, so the title is not repeated on screen; the page's own
 * content is its first visible thing. Optional tools sit on a quiet line at the top.
 */
export function PageHead({
  title,
  children,
}: {
  title: string
  children?: React.ReactNode
}): React.JSX.Element {
  return (
    <>
      <h1 id="page-heading" tabIndex={-1} className="sr-only">
        {title}
      </h1>
      {children === undefined ? null : (
        <div className="page-tools">{children}</div>
      )}
    </>
  )
}
