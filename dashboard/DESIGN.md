# Dashboard design

## Brief

A private, always-on household iPad in a Copenhagen home. It is read from across the room
far more often than it is touched. The job: know the time, the weather and the state of the
household lists at a glance, then reach any control in one tap. English copy and formats (en-GB dates and times).

## Concepts considered

|             | Idea                                                                                                   | Anchor                                                                      | Signature                                               | O/F/Fe/P      |
| ----------- | ------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------- | ------------------------------------------------------- | ------------- |
| A. Formal   | The house is a directory: one typeset list where every line is a part of the home with its live value  | Dashboard Home is defined as an overview where selecting a feature opens it | the selected line becomes the page title                | 3 / 5 / 5 / 3 |
| B. Place    | The screen is lit by the Copenhagen sun: paper by day, ink by night, a horizon line with the sun on it | an always-on display in a Copenhagen room; every time is Europe/Copenhagen  | the sun travels the horizon, the whole UI turns at dusk | 4 / 5 / 4 / 2 |
| C. Material | A wall of light switches: every feature is a physical plate                                            | the home controls                                                           | plates depress on touch                                 | 2 / 3 / 4 / 4 |

Picked **A + B**: a directory of the home, lit by the Copenhagen sun. A gives structure
and the mkdir.directory reference; B gives the idle life and a reason for two themes.

## Concept card

```
Idea:        The home as a directory, lit by the Copenhagen sun.
Metaphor:    A printed index on a wall that daylight falls across.
Signature:   The horizon: a hairline across every page with the sun at its true position
             between sunrise and sunset. The theme follows it (paper by day, ink at night).
             Opening a line morphs its name into the page title (view transition).
Emotion:     Calm, legible, quietly alive. Like a good clock.
Motion verb: Rise. Things come up from a baseline; hovers lift; the sun climbs.
Screenshot:  Home at night: giant clock, the sun below the horizon as a hollow ring,
             the directory of the house in large type with live values on the right.
```

## System

- **Palette (day / night)**: ground `#F4F4F0` / `#0B0C0E`; ink `#121418` / `#ECECE6`;
  muted `#5A5F69` / `#9A9EA6` (5.8:1 and 7.3:1 on the ground); wash and wash-strong are ink
  at 5 % / 9 % (night 6 % / 11 %). Sun `#FF5B1F`, used only as a fill: the sun disc, the
  wordmark dot, today, the current hour, high priority, the lit bulb, a chart series.
  Never body text. Errors `#B42318` / `#FF8A7A`.
- **Type**: Familjen Grotesk for everything, including numerals (tabular figures). No mono.
  Display tracking -0.045em to -0.06em, leading 0.86 to 0.95. Body 16 to 18 px.
- **Grid**: 12 columns, `clamp(1.25rem, 3.2vw, 2.5rem)` outer margin. The shell column is
  `minmax(0, 1fr)` so no page can push the viewport wider than the phone.
- **Shape**: keys are circles or capsules; media are 0.5rem to 0.75rem. Separation is
  whitespace and tone steps (ground, wash, wash-strong, ink). No hairlines inside pages
  (the horizon is the only line), no shadows, no glows, no blur, no gradients.
- **Motion**: one ease `cubic-bezier(.2,.7,.1,1)`; 180 ms small, 420 ms panels, 700 ms page.
  Things rise from a baseline. Press: 0.97 scale. Reduced motion: no transitions.
- **Idle**: the sun on the horizon (minute clock) and the clock. Nothing else moves at rest.

## Round 2: say it with the thing itself

Owner feedback: Home should be visuals, not sentences; no text for control state; no
"Home connected"; Photos is only the album; less text everywhere.

- **One h1 per page, usually sr-only.** The top bar already says where you are, so pages
  carry no visible title, eyebrow or "Back to Home". Section headings that only repeat
  what the content shows are sr-only too.
- **Removed words move to assistive technology, not away.** Explanations, units already
  shown by an icon, counts and captions become `sr-only` spans or `aria-label`s. Icon-only
  keys keep their label as an sr-only span, so their accessible name (and text content) is
  unchanged.
- **Glances are visuals.** Home shows the clock, the sun's arc, the temperature with a
  seven-day bar chart, a row of key plates (the keys name their device: a lit bulb, a TV
  power key; only the speaker plate carries a glyph), today's Charge, Rest and Effort
  rings on the bare paper (each named by a glyph), and a progress plate: four weeks of
  exercise lines with three featured exercises in colour, named only by a legend.
- **Control state is never claimed in text.** Keys show only commands the device reported.
  A sent command draws on the key itself: pending is a sun arc spinning round it, accepted
  is a small acknowledge bump with a sun ring rippling out; a failure shakes the key, rings
  it red and shows a terse tag ("No answer", "Key rejected") from `command-feedback.ts`.
  Nothing says "on" or "off".
- **Success is a visual acknowledgement; failure stays visible.** Saves, refreshes and
  confirmations are announced in an sr-only status region; the control itself acknowledges
  (spinner on refresh keys, pressed tone). Failures show a red dot plus one short line.
- **Missing data is a dash.** `data-gap="true"` draws an en dash while assistive technology
  still reads "Unavailable". Weather never invents a value; dry hours (0 mm) and chances
  under 20 % are quiet (sr-only) so rain stands out.
- **Provenance as marks.** Training sync shows a freshness ring (filled sun when confirmed)
  and one chip per stream (filled dot when covered); the full sentence is sr-only.
- **Refresh lives top right** as a round key on every data page (Photos, Wishlist,
  Watchlist). Workout and Recovery have none (see below). Weather has none: it refreshes itself on the forecast's
  validity, and a stale read shows a rewind mark beside the current figures.
- **Workout is Log, Progress and Body** behind one segmented switcher (wash track, ink pill) that
  every segmented control on the page shares, with the period choice on the right. There is no
  refresh key: changes reload the view themselves. Log is one plate per training session. A logged
  Workout and a strap session on the same Copenhagen day become one plate (exercises left; effort
  ring, stats, heart-rate trace and zone bar right) under `lib/session-log.ts`; a strap session
  alone shows its own stats, and a map only when it has a route; a logged Workout alone shows
  its exercises. Progress opens on 4 weeks, tone bands instead of grid lines. Body leads with the
  current weight large, the delta over the range, one calm chart of alternating tone bands, and the
  profile values as quiet plates with a pencil key; it fits 1280x720.
- **Controls is three plates on one screen**, the same tone as Home's deck: the lights as a
  tall wall switch (lit bulb on ink above, dark bulb on paper below), the TV remote in the
  middle and the speaker's narrow remote on the right. The device glyph names each remote;
  keys sit on paper inside the plate, and both volume rockers put + on top.
- **Weather is one screen**: the figures on the left (now, then the week as rows whose ranges
  share one scale), and the next 24 hours as a horizon on the right: the temperature line,
  rain as columns beneath it, the nights as darker ground from the same sun model that turns
  the theme, and the sun on the line at this minute. Every hourly value stays in a table
  for assistive technology.
- **Recovery is the day's rings and its night**: Charge, Rest and Effort as three big rings on
  the paper with the week of days (the cartwheel) beneath them, and the night on a plate beside
  them as a hypnogram: bedtime to wake across, a lane per stage (awake, REM, light, deep) with its
  total beside it, every stretch a solid bar in the stage's colour and alternate hours in a darker
  tone instead of grid lines. Nothing says "refresh"; re-tapping the chosen day reads it again, and
  a rewind mark with the last push shows only when the sync is not confirmed. Body signals, the
  journal and the strap's workouts follow on scroll; logged workouts stay on Workout. Portrait and
  phone stack rings, week and night. Without ordered stages the night falls back to its totals.
- **Travel is the map**: full-bleed from the top bar to the bottom edge, with every control a
  ground plate floating on it. The planner is one capsule docked bottom-left (walk and run
  glyphs, 5 km, 10 km, Custom and Steps, the inline field, then the ink "Plan loops" key); the
  loops rise above it as one plate of rows whose selected loop wears the route's sun mark;
  2D/3D, zoom and locate are one tall plate centred on the right edge. The origin is the
  marker (only the demo origin gets a chip), and the one small attribution sits bottom-right,
  merged with the routing credit. In portrait the planner spans the bottom and the controls
  move to the top of the right edge; on a phone the attribution moves to the top-left.
- **Tap targets are at least 44 px** (key-icon 48 px, key-round 64 px, key-hero 96 px);
  inline attribution links are the only exception.
- **Connection** is one word in display type beside a signal disc that pulses once (and
  shakes when not connected), a big check-again key and "Key stays on the server".
  The top bar shows a neutral plug glyph, never a state.

## Genericness test

A generic model would ship a dark dashboard of rounded cards with a yellow accent (the old
design). Changed: no cards, a light theme by day, an index list as the home, one orange used
only as the sun, a real astronomical sun position as the identity element.
