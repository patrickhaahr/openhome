import { getRouteApi, useRouter } from "@tanstack/react-router"
import { useState } from "react"
import { Segments } from "@/components/list-parts"
import { remember } from "@/components/nav-layout"
import { dataModeCookie, type DataMode } from "@/lib/demo/data-mode"

const options = [
  { value: "live", label: "Live data" },
  { value: "demo", label: "Demo data" },
] as const satisfies ReadonlyArray<{ value: DataMode; label: string }>

const rootRoute = getRouteApi("__root__")

export function DataModeSwitch(): React.JSX.Element {
  const router = useRouter()
  const [mode, setMode] = useState(rootRoute.useLoaderData().dataMode)

  function choose(next: DataMode): void {
    remember(dataModeCookie(next))
    setMode(next)
    void router.invalidate()
  }

  return (
    <Segments
      fit
      label="Health and workout data"
      value={mode}
      options={options}
      choose={choose}
    />
  )
}
