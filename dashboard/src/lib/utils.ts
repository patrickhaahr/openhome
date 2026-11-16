import { createCn } from "cn/config"

/* Mirrors the app tokens registered in `@theme inline` (styles.css). Without them the merge
   cannot tell `text-ink` (colour) from `text-mega` (size) and drops one of two sibling classes. */
export const cn = createCn({
  extend: {
    theme: {
      color: [
        "ground",
        "raised",
        "ink",
        "muted-ink",
        "line",
        "line-strong",
        "wash",
        "wash-strong",
        "sun",
        "sun-glow",
        "danger",
        "scrim",
        "connected",
      ],
      text: [
        "mega",
        "display",
        "title",
        "lead",
        "control",
        "label",
        "caption",
        "micro",
        "figure-sm",
        "figure-md",
        "figure-lg",
        "score",
        "link-status",
        "lost-code",
        "aside",
      ],
      leading: ["status", "lost-code"],
      tracking: ["score", "score-tight", "caption", "status", "lost-code"],
      spacing: ["margin", "bar", "ring-baseline", "glance-ring"],
    },
  },
})
