export function NightScreen({
  ref,
  waking,
}: {
  ref: React.Ref<HTMLDivElement>
  waking: boolean
}): React.JSX.Element {
  return (
    <div
      ref={ref}
      data-slot="idle-night"
      className="fixed inset-0 z-100 cursor-none touch-none bg-black outline-none select-none touch-callout-none data-[waking=true]:opacity-0 data-[waking=true]:transition-opacity data-[waking=true]:duration-mid data-[waking=true]:ease-app"
      data-waking={waking}
      role="dialog"
      aria-modal="true"
      aria-label="Night screen. Touch the screen or press any key to return."
      tabIndex={-1}
    />
  )
}
