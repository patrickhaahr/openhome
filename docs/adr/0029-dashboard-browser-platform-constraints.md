# Dashboard browser and styling constraints

The Dashboard runs on an older wall iPad and on the photo frame's Chrome 106
WebView. It is server-rendered and styled with Tailwind utilities. These
platform limits explain choices that look unnecessary in a modern desktop
browser.

- **Cookies through `document.cookie`.** The server reads the currency, layout
  and label cookies before first paint. They are therefore written with
  `document.cookie`, because older iPad Safari lacks the Cookie Store API.
- **Server and browser renders must match.** Backlight dial coordinates are
  rounded to two decimals in `dashboard/src/lib/backlight-light.ts`, so both
  renders produce identical SVG attribute strings. Hydration first renders the
  server's `sun` mode; a stored household choice that has already painted wins
  over it. Server-rendered thumbnails can finish loading before React attaches
  their listeners, so `photo-tile.tsx` checks the loaded state on mount.
- **Calendar dates are not instants.** Training Days are calendar dates. They
  are formatted at UTC midnight, so a timezone never shifts the day, and their
  arithmetic is calendar arithmetic, because elapsed local hours are wrong
  across DST. The next Copenhagen midnight is found by instant, because a local
  day can last 23, 24 or 25 hours.
- **Pairing a strap session is presentation only.** Axum never matches a logged
  Workout with a NOOP strap session, and a logged Workout has only a date.
  `dashboard/src/lib/session-log.ts` pairs them only within the same Copenhagen
  Training Day and only for sports that cover no ground. Pairing
  is one-to-one: the longest sessions win, and the earlier start breaks a tie.
  Remaining candidates pair in order of logging and strap start. Strap routes
  are shown only for unpaired sessions.
- **Touch is not mouse.** Browsers set `event.detail` to 2 on a mouse double
  click but not on touch taps, so a touch double tap on the logo is timed by
  hand. The click that a drag's release produces arrives in the same task. The
  flag that suppresses it must be cleared if that click never comes, or it
  would swallow a later genuine click. A mouse drag across Binder card details
  selects text; that is reading, not a swipe to the neighbouring card.
- **The turning leaf belongs to JavaScript.** JavaScript writes the turning
  Binder leaf's inline transform while it is lifted. No transform or transition
  utility may be applied to it.
- **Scrollbars never show on the wall screen.** On the wall screen and the
  iPad, scrolling stays but scrollbars are hidden, so no layout narrows when
  one would appear.
- **Reduced motion stops the theme turn.** When the sun changes the theme, the
  room's colours cross-fade over 1.2 s, never under reduced motion. The daylight
  provider toggles the `theme-turning` class on the `html` element.
- **Tailwind needs the app's tokens.** The `cn` merge config in
  `dashboard/src/lib/utils.ts` lists the custom theme tokens from `styles.css`.
  Without them it cannot tell text colour tokens from text size tokens, and it
  drops one of two sibling classes. Each token's utility must replace a
  built-in of its kind and keep a sibling of another kind that shares its
  prefix, such as `text-ink` beside `text-mega`. App-wide tokens live in
  `styles.css` and each area's tokens in `<area>.theme.css`. Generated client
  assets are kept out of the SSR source scan through an explicit `source` root.
- **Some utilities are written by hand.** `mask-r-from-*` places its stop as a
  percentage from the start edge, never the end, so the fade over the last 2rem
  of a scrolling strip is hand-written. A utility cannot `@apply` another, so
  the missing-value dash rule is repeated where needed.
- **The missing-value dash stays out of the accessible name.** The dash uses an
  empty alternative text after the slash, so assistive technology keeps the
  words but not the dash. When the host's own text is the missing-value word,
  `font-size: 0` hides it and is `!important` to beat the host's size classes.
  The trend chart SVG is decoration: the summary and table carry the values.
- **Number formatting avoids minus zero.** Axis ticks add zero after `toFixed`,
  so a rounded negative zero shows as 0.
- **Router active state is not trusted for switches.** A Food page switch link
  is current only for its exact search. The Router's own active state could
  mark a second choice beside the chosen one, so it is not used.
- **Storage may be missing.** `sessionStorage` can be disabled in private
  browsing, so reads and writes of pending drafts swallow errors. The mounted
  flow keeps its retry identity in memory (see `docs/wishlists.md`).
- **Retired routes redirect.** The retired Today recovery view only redirects.
  Old links to it, or to a day on it, keep their day and selection.
