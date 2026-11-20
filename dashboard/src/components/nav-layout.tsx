import {
  createContext,
  use,
  useCallback,
  useMemo,
  useRef,
  useState,
} from "react"
import { Segments } from "@/components/list-parts"
import {
  isDoubleTap,
  navLabelsCookie,
  navLayoutCookie,
  otherLayout,
  type NavLabels,
  type NavLayout,
  type NavPrefs,
} from "@/lib/nav-layout"

type NavLayoutState = {
  readonly layout: NavLayout
  readonly labels: NavLabels
  readonly choose: (layout: NavLayout) => void
  readonly chooseLabels: (labels: NavLabels) => void
}

const NavLayoutContext = createContext<NavLayoutState | null>(null)

function remember(cookie: string): void {
  // oxlint-disable-next-line unicorn/no-document-cookie -- see remember's comment.
  document.cookie = cookie
}

export function NavLayoutProvider({
  initial,
  children,
}: {
  initial: NavPrefs
  children: React.ReactNode
}): React.JSX.Element {
  const [layout, setLayout] = useState(initial.layout)
  const [labels, setLabels] = useState(initial.labels)

  const choose = useCallback((next: NavLayout) => {
    remember(navLayoutCookie(next))
    setLayout(next)
  }, [])

  const chooseLabels = useCallback((next: NavLabels) => {
    remember(navLabelsCookie(next))
    setLabels(next)
  }, [])

  const state = useMemo(
    () => ({ layout, labels, choose, chooseLabels }),
    [layout, labels, choose, chooseLabels]
  )

  return <NavLayoutContext value={state}>{children}</NavLayoutContext>
}

export function useNavLayout(): NavLayoutState {
  const state = use(NavLayoutContext)

  if (state === null) throw new Error("useNavLayout needs a NavLayoutProvider")

  return state
}

export const logoHintId = "logo-hint"

export function useLogoTaps(): (event: React.MouseEvent) => void {
  const { layout, choose } = useNavLayout()
  const previous = useRef<number | undefined>(undefined)

  return (event) => {
    const now = Date.now()

    if (event.detail === 2 || isDoubleTap(previous.current, now)) {
      event.preventDefault()
      previous.current = undefined
      choose(otherLayout(layout))

      return
    }

    previous.current = now
  }
}

const options = [
  { value: "top", label: "Top bar" },
  { value: "side", label: "Sidebar" },
] as const satisfies ReadonlyArray<{ value: NavLayout; label: string }>

const labelOptions = [
  { value: "words", label: "Words" },
  { value: "icons", label: "Icons" },
  { value: "both", label: "Both" },
] as const satisfies ReadonlyArray<{ value: NavLabels; label: string }>

export function NavLayoutSwitch(): React.JSX.Element {
  const { layout, labels, choose, chooseLabels } = useNavLayout()

  return (
    <div className="flex flex-wrap gap-x-6 gap-y-3">
      <Segments
        fit
        label="Navigation"
        value={layout}
        options={options}
        choose={choose}
      />
      <Segments
        fit
        label="Top bar tabs"
        value={labels}
        options={labelOptions}
        choose={chooseLabels}
      />
    </div>
  )
}
