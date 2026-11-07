# Mobile design

## Idea

**The hole in a weight plate is the door of the house.** One app for lifting and for the home, in a dark
house with one lamp: everything is black until it is lit, and only light is yellow.

- **The mark** is that plate, end on, with a house-shaped hole.
- **The Workout screen is the plate**, larger than the phone and hanging off its right edge, with the last
  Workout's Exercises cast into the rim. Tapping it repeats them. An unfinished Workout turns it dark, and it
  lights one segment per Set confirmed.
- **A confirmed Set lights up.** The whole row turns yellow; nothing else on the screen is as bright.
- **The Backlight is drawn as itself**: a dark TV with the requested light behind it. This is the one place
  colours other than yellow and gradients are allowed, because the colour is the thing.

## Words

Symbols first. A word stays on screen only when it is data (an Exercise, a date, a Muscle Group) or when
something went wrong (one line, the rest a tap away). **Every removed word moves to the accessibility name,
not away**: `Spoken` and key descriptions keep the full sentence, which TalkBack reads and the device tests
find. Never remove a label without giving its control a description.

## System

- **Colour** (`ui/theme/Theme.kt`): ground `#000`; surfaces step up by lightness alone (`Slab`, `Raised`,
  `Lifted`), with no borders or shadows; `Lamp` `#FFD60A` is the only accent; `Alarm` only for failure.
- **Type**: Familjen Grotesk with tabular figures, the same face as the Dashboard and the TV. Five sizes, from
  `Giant` for the one number a screen is about down to `Small` for units.
- **Symbols** (`ui/design/Glyph.kt`): the app's own set, on one 24-unit grid with one stroke and round ends.
  One meaning per symbol.
- **Keys** (`ui/design/Keys.kt`): discs and capsules at least 44 dp; press depth and a haptic tick throughout.
- **Commands are not states**: a sent command draws on its key (arc while sending, a ring on
  acknowledgement, a shake and red ring on failure) and nothing ever says on or off.
- **Motion**: one ease, `cubic-bezier(.2,.7,.1,1)`; screens rise 24 dp; the plate rolls in once. Nothing else
  moves at rest except a Backlight effect, which is what that effect does.
