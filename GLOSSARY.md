# Native OpenHome Mobile

Terms specific to the replacement mobile experience. Shared household and fitness terms remain defined in [CONTEXT.md](CONTEXT.md).

## Navigation

**Home Controls**:
The TV remote, speaker, lights and TV backlight controls grouped in the native app's Home tab.
_Avoid_: Server controls, household overview

**Workout Tab**:
The native app's initial destination and first tab, containing workout start/resume, history, the exercise library and exercise creation.
_Avoid_: Progress tab, workout detail

## Workout logging

**Active Workout**:
A workout being logged that can be left and resumed without losing its recorded exercises or sets.
_Avoid_: Finished workout, template

**Completed Set**:
A set the user has explicitly marked as performed during an Active Workout.
_Avoid_: Planned set, prefilled set

**Repeated Workout**:
A new Active Workout prefilled from a past workout's exercise order and set values, with all sets initially uncompleted.
_Avoid_: Completed copy, automatically logged workout

**Complete Exercise**:
An explicit confirmation that the displayed sets were performed, followed by advancing to the next exercise. Merely navigating to another exercise is not confirmation.
_Avoid_: Skip exercise, automatic completion
