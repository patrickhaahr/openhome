# Cinema picture baseline

Applied and read back on the LG 55NANO806PA, firmware 03.53.45, on 21 October 2026.
Scope: `input: default`, `dynamicRange: sdr`, `pictureMode: cinema`, `2d`.
This is a reference-oriented evening-viewing baseline, not a measured calibration.
No colorimeter, emitted-light measurement or physical clipping-pattern check was
available. D65, luminance, gamma tracking and colour error have not been measured.

## Settings

| Control | Before | Cinema now |
| --- | --- | --- |
| Backlight | 100 | 50 |
| Brightness / black level | 65 | 50 |
| Contrast | 65 | 80 |
| Colour / saturation | 60 | 50 |
| Tint | 0 | 0 |
| White-balance preset | Warm3 | Warm2 |
| Two-point RGB gains | 0 / −10 / −30 | 0 / 0 / 0 |
| RGB offsets | 0 / 0 / 0 | 0 / 0 / 0 |
| Gamma | 2.2 (`medium`) | 2.4 (`high1`) |
| Colour gamut | Auto | Auto |
| Sharpness | 10 | 0 |
| Dynamic colour | Medium | Off |
| Dynamic contrast | Off | Off |
| Noise reduction | Low | Off |
| MPEG NR / Super Resolution / Smooth Gradation | Off | Off |
| Local dimming | Medium | Off |
| Real Cinema | On | On |
| TruMotion | Not captured before with app context | Off |
| Motion Pro / Motion Eye Care | Off | Off |
| Reduce Blue Light, global | On | Off |
| Energy Saving, global | Auto | Off |
| Energy-saving manual-change flag, global | False | True |

Warm2 replaces the deliberately amber night tint. Two-point gains/offsets are
neutral; existing multipoint white-balance arrays and CMS corrections remain zero.
No per-panel corrections were copied from someone else's TV.

Contrast 80, brightness 50, colour 50 and Warm2 also match this firmware's ordinary
LCD Cinema defaults in `/etc/palm/defaultSettings.json`. OLED-specific defaults
in the same file are conditional and were not used. Gamma 2.4 is an evening-room
choice; the firmware's `high1` ten-point luminance table follows x^2.4. It is not
evidence that the physical panel tracks that curve exactly.

Backlight 50 is a moderate starting point, not a claimed 100-nit measurement.
Adjust backlight for room light; keep brightness 50 unless a black-level pattern
shows a reason to change it. Gamut Auto selects the content's colour space.
Sharpness 0 and disabled enhancement/noise processing preserve source detail and
film grain. Local dimming is off as a conservative choice for stable luminance;
its visible black-level/blooming trade-off still needs inspection on this panel.
This does not eliminate the LCD's native black-floor limitation.

Setting only `energySaving: off` initially read back correctly but reverted to
Auto after switching profiles. Setting the supported global
`energySavingModified: true` flag with it kept Off through the remote toggle
round trip. No recurring override or launcher code change was needed.

TruMotion off avoids interpolation; Real Cinema stays on for film cadence.
The legacy `edgeEnhancer` key remains `on`: this firmware rejects writes to it
and does not expose its descriptor. No descriptor or service policy was modified
to force it. Sharpness is independently set to 0 and verified.

## Night profile and scope

The eye still switches only `pictureMode`. Expert Dark Room (`expert2`) keeps
Warm3, backlight 30, brightness 65, contrast 65, colour 60 and RGB gains 0/−15/−40.
All 78 readable profile-scoped controls matched its pre-change snapshot. The
global blue-light and energy-saving switches are off in both profiles, so its
stored white balance supplies the warmth. Its physical appearance can differ
from before because these switches previously affected both profiles.

Other inputs and HDR picture presets were not rewritten. The two global switches
also affect those contexts. SDR values should not be blindly copied into HDR.

## Verification and evidence

All 26 submitted ordinary profile controls, both global switches and the
energy-saving manual-change flag were read back. TruMotion is a mixed/per-app
key: `getSystemSettings` requires
`current_app: true` or an `app_id`; ordinary reads omit it. Separate reads for
Home, `youtube.leanback.v4` and `netflix` returned `off` in this Cinema context.
This checks stored settings, not actual playback cadence.

The actual remote eye was activated twice. Native reads confirmed Expert Dark
Room with all 78 stored profile controls unchanged, then Cinema with its settings
restored and both global controls still off. Cinema was left active.
Ignored local evidence is under `private/cinema-reference-*`, including the full
before/after snapshots, submitted values and remote readbacks. A software display
capture cannot validate the panel's emitted colour, backlight or white point.
See [rollback](ROLLBACK.md#undo-the-cinema-baseline).

## Sources and limits

- [LG's exact model specifications](https://www.lg.com/gr/tileoraseis/lg-55nano806pa)
  establish the model; no trustworthy measured exact-model calibration was found.
- [LG's picture-settings guidance](https://www.lg.com/us/support/help-library/best-picture-settings-for-your-lg-4k-or-4k-oled-tv-CT10000020-20150577528034)
  explains the effect of room lighting and the difference between picture modes.
- [RTINGS' NANO81 measurements/settings](https://www.rtings.com/tv/reviews/lg/nano81/settings)
  support a nearby NanoCell baseline: Warm2, Auto gamut, brightness 50 and disabled
  automatic processing. This is a different model/year, not proof of exact-panel
  accuracy. Their panel-specific white-balance values were not copied.
- [LG's settings-service reference](https://www.webosose.org/docs/reference/ls2-api/com-webos-service-settings/)
  documents app-specific settings queries. The running TV's descriptors and
  readbacks determined which controls and values were actually supported.
