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
      ],
      text: ["mega", "display", "title", "lead"],
      spacing: ["margin", "bar"],
    },
  },
})
