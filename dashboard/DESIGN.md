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
  Left untouched, the **Idle Slideshow** covers the page: the selected photos on the night
  ground, crossfading slowly with a gentle drift (crossfade only with reduced motion), and
  one small, half-clear night-ground tone plate in the bottom-left corner holding the clock,
  the date and today's weather, so the photo shows through. Flat tint only: no gradient,
  shadow, blur or glass behind the text. Later idle
  content (important dates) joins the same plate. Without photos the plate sits on the
  bare ground. The first touch only wakes. From 23:00 to 08:30 the Night Screen takes its
  place: plain black, nothing on it.

Round 2 and the genericness test live on their own page.

- [Dashboard design: round 2](DESIGN_ROUND_2.md) covers round 2 (say it with the thing itself) and the genericness test.
