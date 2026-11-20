export type Viewport = {
  readonly width: number
  readonly height: number
  readonly scale: number
}

export const viewports = {
  "1920x1080": { width: 1920, height: 1080, scale: 1 },
  "1536x864": { width: 1536, height: 864, scale: 1 },
  "1280x800": { width: 1280, height: 800, scale: 1 },
  "1024x640": { width: 1024, height: 640, scale: 1 },
  "1180x820@2": { width: 1180, height: 820, scale: 2 },
  "820x1180@2": { width: 820, height: 1180, scale: 2 },
  "390x844@2": { width: 390, height: 844, scale: 2 },
} as const satisfies Record<string, Viewport>

export type ViewportName = keyof typeof viewports

export type Theme = "day" | "night"

export const themes: readonly Theme[] = ["day", "night"]

type Role =
  | "button"
  | "combobox"
  | "dialog"
  | "heading"
  | "link"
  | "list"
  | "option"
  | "region"

export type Target =
  | { readonly role: Role; readonly name: string; readonly exact?: boolean }
  | { readonly text: string }

export type Step =
  | { readonly kind: "click"; readonly target: Target }
  | { readonly kind: "press"; readonly key: string }
  | {
      readonly kind: "hold-drag"
      readonly target: Target
      readonly from: number
      readonly to: number
    }

export type Scenario = {
  readonly id: string
  readonly path: string
  readonly viewports: readonly ViewportName[]
  readonly expect: Target
  readonly steps?: readonly Step[]
  readonly capture?: "full" | "viewport"
  readonly motion?: "reduce" | "no-preference"
}

const every: readonly ViewportName[] = [
  "1920x1080",
  "1536x864",
  "1024x640",
  "1180x820@2",
  "820x1180@2",
  "390x844@2",
]

const everyAndLaptop: readonly ViewportName[] = [...every, "1280x800"]

const tablets: readonly ViewportName[] = ["1180x820@2", "820x1180@2"]

const states: readonly ViewportName[] = [
  "1920x1080",
  "1180x820@2",
  "820x1180@2",
  "390x844@2",
]

const heading = (name: string): Target => ({ role: "heading", name })

const button = (name: string): Target => ({ role: "button", name })

const dialog = (name: string): Target => ({ role: "dialog", name })

const click = (target: Target): Step => ({ kind: "click", target })

const firstPage: Target = { role: "list", name: "Page 1", exact: true }

const rightPage: Target = { role: "list", name: "Page 2", exact: true }

export const scenarios: readonly Scenario[] = [
  {
    id: "home",
    path: "/",
    viewports: everyAndLaptop,
    expect: { role: "link", name: "Open Weather" },
  },
  {
    id: "weather",
    path: "/weather",
    viewports: every,
    expect: heading("Next 24 hours"),
  },
  {
    id: "controls",
    path: "/controls",
    viewports: every,
    expect: button("Channel up"),
  },
  {
    id: "backlight",
    path: "/backlight",
    viewports: every,
    expect: button("TV Sync"),
  },
  {
    id: "travel",
    path: "/travel",
    viewports: every,
    expect: button("Plan loops"),
  },
  {
    id: "travel-loops",
    path: "/travel",
    viewports: states,
    steps: [click(button("Plan loops"))],
    expect: button("Select Loop A"),
  },
  {
    id: "workout",
    path: "/workout",
    viewports: every,
    expect: heading("Sessions"),
  },
  {
    id: "workout-progress",
    path: "/workout?view=progress",
    viewports: states,
    expect: heading("Exercise progress"),
  },
  {
    id: "workout-body",
    path: "/workout?view=body",
    viewports: states,
    expect: heading("Body-weight history"),
  },
  {
    id: "recovery",
    path: "/recovery",
    viewports: every,
    expect: heading("Recovery day"),
  },
  {
    id: "food",
    path: "/food",
    viewports: everyAndLaptop,
    expect: button("All nutrients"),
  },
  {
    id: "food-nutrients",
    path: "/food",
    viewports: states,
    steps: [click(button("All nutrients"))],
    expect: button("All nutrients"),
  },
  {
    id: "food-trends",
    path: "/food?view=trends",
    viewports: states,
    expect: heading("Energy by macronutrient"),
  },
  {
    id: "lists",
    path: "/lists",
    viewports: everyAndLaptop,
    expect: button("Add a wish"),
  },
  {
    id: "lists-wish-sheet",
    path: "/lists",
    viewports: states,
    steps: [click(button("Merino hiking socks"))],
    expect: dialog("Merino hiking socks"),
    capture: "viewport",
  },
  {
    id: "lists-wish-add",
    path: "/lists",
    viewports: states,
    steps: [click(button("Add a wish"))],
    expect: dialog("New wish"),
    capture: "viewport",
  },
  {
    id: "lists-household",
    path: "/lists",
    viewports: states,
    steps: [click(button("Household and lists"))],
    expect: dialog("Household and lists"),
    capture: "viewport",
  },
  {
    id: "lists-watch",
    path: "/lists?tab=watch",
    viewports: states,
    expect: button("Add a title"),
  },
  {
    id: "lists-watch-sheet",
    path: "/lists?tab=watch",
    viewports: states,
    steps: [click(button("Project Hail Mary"))],
    expect: dialog("Project Hail Mary"),
    capture: "viewport",
  },
  {
    id: "lists-watch-add",
    path: "/lists?tab=watch",
    viewports: states,
    steps: [click(button("Add a title"))],
    expect: dialog("Add a title"),
    capture: "viewport",
  },
  {
    id: "lists-dates",
    path: "/lists?tab=dates",
    viewports: states,
    expect: button("Add a date"),
  },
  {
    id: "lists-date-sheet",
    path: "/lists?tab=dates",
    viewports: states,
    steps: [click(button("Robin's birthday"))],
    expect: dialog("Robin's birthday"),
    capture: "viewport",
  },
  {
    id: "lists-date-add",
    path: "/lists?tab=dates",
    viewports: states,
    steps: [click(button("Add a date"))],
    expect: dialog("Add a date"),
    capture: "viewport",
  },
  {
    id: "binder",
    path: "/binder",
    viewports: everyAndLaptop,
    expect: firstPage,
  },
  {
    id: "binder-card-first",
    path: "/binder",
    viewports: states,
    steps: [click(button("Tamsin Reefwalker (003)"))],
    expect: dialog("Tamsin Reefwalker (003)"),
    capture: "viewport",
  },
  {
    id: "binder-card-second",
    path: "/binder",
    viewports: states,
    steps: [click(button("Orrin Saltgate (004)"))],
    expect: dialog("Orrin Saltgate (004)"),
    capture: "viewport",
  },
  {
    id: "binder-add",
    path: "/binder",
    viewports: states,
    steps: [click(button("Add card"))],
    expect: dialog("Add a card"),
    capture: "viewport",
  },
  {
    id: "binder-currency",
    path: "/binder",
    viewports: states,
    steps: [click({ role: "combobox", name: "Currency" })],
    expect: { role: "option", name: "EUR" },
    capture: "viewport",
  },
  {
    id: "binder-second-spread",
    path: "/binder",
    viewports: states,
    steps: [click(button("Next pages"))],
    expect: { role: "list", name: "Page 3", exact: true },
  },
  {
    id: "binder-empty",
    path: "/binder",
    viewports: tablets,
    steps: [click(button("Sam"))],
    expect: { text: "Sam’s Binder is empty" },
  },
  {
    id: "photos",
    path: "/photos",
    viewports: every,
    expect: button("Coastal walks"),
    capture: "viewport",
  },
  {
    id: "photos-album",
    path: "/photos",
    viewports: states,
    steps: [click(button("Coastal walks"))],
    expect: heading("Coastal walks"),
    capture: "viewport",
  },
  {
    id: "connection",
    path: "/connection",
    viewports: every,
    expect: heading("Connection status"),
  },
  {
    id: "lost",
    path: "/nope",
    viewports: every,
    expect: heading("Page not found"),
  },
  {
    id: "idle",
    path: "/?idle=now",
    viewports: every,
    expect: { role: "region", name: "Now" },
  },
  {
    id: "nav-more",
    path: "/",
    viewports: ["390x844@2"],
    steps: [click(button("More"))],
    expect: dialog("More pages"),
    capture: "viewport",
  },
  {
    id: "motion-home",
    path: "/",
    viewports: tablets,
    expect: { role: "link", name: "Open Weather" },
    motion: "no-preference",
  },
  {
    id: "motion-binder",
    path: "/binder",
    viewports: tablets,
    expect: firstPage,
    motion: "no-preference",
  },
  {
    id: "motion-binder-lifted",
    path: "/binder",
    viewports: tablets,
    steps: [{ kind: "hold-drag", target: rightPage, from: 0.8, to: 0.2 }],
    expect: firstPage,
    capture: "viewport",
    motion: "no-preference",
  },
  {
    id: "motion-lists-sheet",
    path: "/lists",
    viewports: tablets,
    steps: [click(button("Merino hiking socks"))],
    expect: dialog("Merino hiking socks"),
    capture: "viewport",
    motion: "no-preference",
  },
]
