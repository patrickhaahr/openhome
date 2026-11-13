# Home Controls

Part of the OpenHome domain language; see [CONTEXT.md](../../CONTEXT.md).

## IR Control

**IR Remote**:
A user-facing control surface that exposes named infrared commands through the Axum API.
_Avoid_: IR device, bridge, blaster

**Command**:
A named IR action that the user triggers from an IR Remote.
_Avoid_: Action, button press, event

## Lights

**Light Command**:
An on or off request sent through the Axum API to the configured SwitchBot
lights. An accepted Light Command acknowledges the request only; it never
establishes the lights' power state.
_Avoid_: Light toggle, IR Command, light state

## Backlight

**Backlight**:
The independently controlled lighting behind the TV. Dashboard copy may also
call it the LED; code and domain names use backlight.
_Avoid_: Lights, Light Command

**TV Sync**:
The Backlight mode whose light output follows the TV picture captured by its
camera, using the household's existing picture-response and black-screen settings.
_Avoid_: Motion detection, TV power state

**Backlight Routine**:
The household's daily schedule for selecting Backlight settings automatically.
_Avoid_: TV power automation, reminder

**Bedtime Mode**:
The Backlight Routine's optional daily time window for showing red as a bedtime
cue, with its own brightness. Outside an enabled window, the routine selects TV Sync.
_Avoid_: Motion mode, midnight power-off

**Backlight Override**:
A manual Backlight selection that temporarily replaces the Backlight Routine
until its next enabled transition or until the household selects Automatic.
_Avoid_: Permanent setting, routine edit

## Mobile Navigation

**Home Tab**:
A mixed overview tab that combines the most-used capabilities from multiple API areas.
_Avoid_: Dashboard endpoint, root route group

**Top-Level Tab**:
A primary mobile navigation destination that intentionally mirrors one Axum API area.
_Avoid_: Arbitrary feature bucket, backend implementation detail

**Remote Button**:
A client-defined control in the Remote tab that maps to exactly one IR Command.
_Avoid_: Dynamic API button, raw endpoint action

**Remote Button Set**:
A curated subset of IR Commands exposed by the Android client as Remote Buttons.
_Avoid_: Full command dump, auto-generated control list

**V1 Remote Button Set**:
The initial Remote Button Set for the single IR Remote: `power`, `bluetooth`, `optical`, `mute`, `volume-up`, and `volume-down`.
_Avoid_: Implicit default buttons, test-only commands

**Unavailable Remote Button**:
A Remote Button that remains visible in the layout but is disabled because its Command is not currently reported as available by the Axum API.
_Avoid_: Hidden button, speculative action

**Home Remote Controls**:
A small subset of the Remote Button Set shown on the Home Tab for quick access to the most-used IR Commands.
_Avoid_: Full remote, duplicate tab

**V1 Home Remote Controls**:
The initial Home Remote Controls: `bluetooth` and `optical`.
_Avoid_: Full remote shortcut set, implicit defaults
