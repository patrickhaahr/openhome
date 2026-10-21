# OpenHome TV

Question: what do I want to watch or change in the room, right now?

## Data and jobs

- One household, viewed across the room on a 55-inch 16:9 TV, usually for seconds.
- The first six LG launch points are the main job; all remaining apps live one Down press away.
- App order and launching remain owned by LG Home, including hold-OK to reorder.
- Four actions: lights on/off and Edifier Bluetooth/optical. Receipt is not device state.
- Time and date use the TV's configured Copenhagen timezone and 24-hour clock.
- Weather is today's Copenhagen-date forecast from Axum: Celsius and millimetres.
- Charge, Rest and Effort are Axum's three derived scores on a 0–100 scale.
- Real zeroes stay zero; missing values are dashes. Failed refreshes are marked stale.
- API data refreshes while Home is visible and when returning from an app.
- Ember is a house-shaped face surrounded by radial voice strokes. It has no audio input.
- OK on Ember toggles a clearly labelled speaking simulation; Back returns to Home.

## Direction

The design skill dealt **True-black flat**, seed **1474990053**. The existing dashboard
keeps its Familjen Grotesk type, warm-white ink `#ecece6`, muted `#9a9ea6`, orange sun
`#ff6a2e`, capsules and restrained horizon. The user's pure-black `#000` replaces the
dashboard's night ground. There are no wallpaper images, gradients, shadows or glass.

The screen's face is OpenHome's small sun beside its wordmark. Its signature is Ember:
a little open house, two orange eyes and a quiet circular instrument that becomes a
radial waveform in the speaking demo. Nothing moves continuously on the Home screen.
The agent animation stops when its screen closes or Home is hidden. Its `reducedMotion`
property stops the looping animation; future TV accessibility integration is separate.

This is a TV, so desktop app guidance for small type and controls does not apply.
At 1920×1080 there are 96px safe gutters, 262×158px app tiles with 130px icons,
168×140px controls, a 174px clock and 184px recovery rings. App labels are always visible.
Semantic recovery colours match the dashboard; application artwork keeps its identity.
There is no phone breakpoint: this native launcher always fills the TV's 16:9 surface.

Shared `TVText.qml` uses native text rendering at the TV's fixed sizes.
Ember's static outline is an authored SVG, compiled to a transparent
PNG because LG's Qt build has no SVG image loader. The file-backed image also avoids
temporary Canvas texture during its animation. No global LG rendering configuration is changed.
LG's screenshot paths intermittently omit unchanged elements (also reproduced with
YouTube's own interface). Review captures visually and retain complete frames. The
compositor-only API does not capture this firmware's native Home surface.
The user confirmed that the physical TV keeps the face and text visible during
the speaking animation; this is a screenshot limitation.

## Evidence

`screenshots/openhome.jpg`, `openhome-apps.jpg`, `ember-idle.jpg` and `ember-speaking.jpg`
are captures of the actual TV, not browser mockups. The public Home screenshot deliberately
uses `bridge/preview.js` synthetic values: weather 12°C, high15/low8, rain0; Charge76,
Rest88, Effort42. They are not personal measurements. The fixture refuses all commands,
has no credential, calls no providers, is never installed by deployment and is removed
after capture. Real API captures and command acceptance evidence remain in ignored `private/`.

`tv-preview-capture` restores the real client in a `finally` block. `tv-client-check`
then verifies authenticated live reads, fresh weather/recovery and the loopback boundary.
