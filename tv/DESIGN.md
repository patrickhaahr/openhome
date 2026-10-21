# OpenHome TV — visual controls

The owner's references are the existing dashboard homepage and a cyan circular
interface. This replaces the first worded design saved in commit `fd3497b`.

## Screen

- Pure black, the dashboard's Familjen Grotesk and warm-white figures.
- Large 24-hour clock; Danish date, for example `onsdag 21. oktober`.
- Today's weather is just its existing dashboard icon and current temperature.
- Three recovery rings use Axum's actual Charge/Rest/Effort scores, with the
  dashboard's filled Lightning, Moon and Heartbeat symbols and colours.
- Two vertical control capsules: lights on/off, then Bluetooth/optical.
- The eye button has the same 104px size and 124px capsule as the other controls. It switches picture profiles: white is the warm/dim Expert Dark Room profile, dark is the reference-oriented Cinema profile; unknown state is dimmed.
- Six large app icons below, without captions. The LG Apps application
  (`com.webos.app.discovery`) is filtered from both the dock and the drawer.
- A down chevron opens the complete app grid; Back or Up from the first row closes it. No drawer X.
- InputHook stays visible. Removed apps disappear through LG’s live app model.
- Opaque app artwork grows to card height, with its edge colour extended across the rounded card. Transparent logos retain their original sizing. This avoids cropping or stretching logos.
- No headings, brand wordmark, prose, app names, tips, weather breakdown or
  attribution text appears on the TV. Date and numerical readings remain.
- Commands acknowledge with a short orange ring; failures use a red warning
  glyph. Focus is a white outline. None of these indicates observed device state.
- Missing measurements remain dashes. Stale readings carry the dashboard's
  rewind icon. Button and app names remain in accessibility metadata only.

The icons are exported directly from `dashboard/node_modules/@phosphor-icons/react`,
using the exact component and weight used by the dashboard. `tv/export-icons.mjs`
renders them through React's static renderer, then `rsvg-convert` compiles PNGs
for LG's Qt build, which has no SVG image loader. The package's MIT license is
included in the TV bundle. This adds no new JavaScript dependency.

The eye button uses Home’s native Luna settings service for `picture.pictureMode`.
It reads before switching between `cinema` and `expert2`, then reads back the
result. A native subscription keeps it current when Input Hook or LG settings
changes the mode; the subscription is cancelled while Home is hidden. A
timeout/error clears the state; periodic/return-to-Home reads recover it.
It changes no individual picture settings when clicked and leaves LG's global
Reduce Blue Light switch alone. The two configured profiles belong to the current
default-input SDR context. Other inputs/HDR profiles are separate.

Cinema uses the [documented film baseline](CINEMA.md): Warm2, neutral RGB gains,
gamma2.4, backlight50, brightness50, contrast80 and colour50, with enhancement
and interpolation controls disabled where supported. Expert Dark Room retains
the previously configured Warm3, backlight30, brightness65, contrast65, colour60,
red gain0, green gain−15 and blue gain−40. Both use zero RGB offsets. Global
Reduce Blue Light and automatic energy saving are now off; the night profile's
warmth comes from its own white balance. The numerical brightness setting
controls black level, so dimming uses the backlight instead.

## Agent

The face/mascot is removed. A cyan core, blue segmented annuli, compass ticks
and fine circuit lines form the new circular mark. Opening it fills the screen.
OK or the play icon runs the speaking simulation: radial strokes react, the
core pulses subtly and an outer ring rotates. Pause stops it; Back/X closes it.
There is no text, fake telemetry, microphone capture, model call or Hermes request.
The animation stops when hidden and respects the existing reduced-motion property.
The cyan/blue palette deliberately follows the owner's agent reference.

## Remote

Left/Right chooses apps. Up reaches the nearest interactive region above the
apps: the agent or right-hand controls. The vertical control rail uses Up/Down;
Left reaches the agent. Down from apps opens the drawer. Hold OK still opens
LG's original reorder interface. Pointer hover/click remains supported.

The red button is globally bound through the existing Input Hook app to the
native Cinema/night switch. It changes the profile without bringing Home to the
foreground. A kernel file lock prevents concurrent shortcut commands. The
helper validates the configured default-input 2D SDR context before writing;
other inputs and HDR are left alone.

## Evidence

`before.jpg` shows LG Home and `after.jpg` shows the current OpenHome launcher.
`openhome-apps.jpg` shows the app drawer; `ember-speaking.jpg` shows the circular
agent concept. These are captures of the actual TV, not browser mockups.
Public weather/recovery values come from `bridge/preview.js`: 12°C, scores76/88/42.
Real API data and live command evidence remain in ignored `private/`.

LG's screenshot paths intermittently omit unchanged elements, also reproduced
in YouTube. The owner confirmed that the physical display stays complete. Retain
complete frames when publishing. The compositor-only API cannot capture this
firmware's native Home surface.
