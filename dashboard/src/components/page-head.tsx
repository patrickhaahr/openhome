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
        <div className="mb-tools-room flex min-h-11 flex-wrap items-center justify-end gap-x-4 gap-y-2">
          {children}
        </div>
      )}
    </>
  )
}
