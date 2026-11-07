# Native Android app design

Status: interview complete; published as [feature spec #226](https://github.com/patrickhaahr/openhome/issues/226) with the `ready-for-agent` label. The user confirmed the app-level and focused API testing boundaries. The requested implementation directory is `mobile/`.

## Confirmed scope

- Build a new OpenHome mobile app from scratch in native Kotlin, with a deliberate focus on UI and UX.
- Consolidate TV remote, speaker, lights and TV backlight controls into one Home tab.
- Include workout logging, Start Workout, workout history, the exercise library and exercise creation.
- Keep Profile.
- Redesign workout logging; the existing experience is painful.
- Remove the Progress tab and all other unrequested features from the new app, including Docker, Server and Away.
- Use the existing Axum API, including the backlight capabilities exposed by the Dashboard's LED page.
- Use exactly two bottom tabs, in order: Workout and Home. Workout is the launch destination. Profile opens from an avatar; Start/Resume Workout, History, Library and exercise creation belong under Workout.
- Focus workout logging on the current exercise, with compact set rows, reps/duration-appropriate inputs, previous performance, repeat-last-set and completion checkmarks. Keep other exercises easy to reach.
- Persist an active workout locally after every change. It must survive process death and restarts and work offline. Synchronize automatically when Wi-Fi and the API are available.
- Use a dark-only interface. The first-round recommendation for restrained color, strong typography, generous spacing and large home-control targets was accepted subject to this dark-only constraint.
- Profile retains dated body-weight logging/history, height, sex, birth date and maximum heart rate.
- Build alongside Expo against the same API and existing fitness data; the native app is intended to deprecate Expo after acceptance. Preserve compatibility without Google Play Services.
- Android system back gestures/buttons must follow the in-app navigation hierarchy. In particular, workout detail returns to History instead of closing the app. Leaving an active workout preserves it.
- Support one active workout, starting empty or repeating a past workout, and backdated logging. Repeating copies exercise order and set values into initially unchecked rows. The user usually repeats the same exercises, order, sets, reps and RPE, with small changes; repeating the last workout should be the primary start action.
- Only checked sets count as performed. On finishing with unchecked rows, summarize what will be omitted and offer return-to-logging or finish. Workout name, notes and RPE remain optional.
- Downloaded history, previous exercise performance and library remain available offline. Exercise creation, workout-history edits, profile changes and body-weight recording are saved locally and queued for sync. Distinguish local changes from synchronized data.
- Design for one user and one active client. Do not add multi-client conflict-resolution UI. Synchronization retries must still avoid creating duplicates.
- Include an optional rest timer after checking a set or completing an exercise, remembered per exercise, with vibration, visibility elsewhere in the app, and skip/adjust/disable actions. Default to 150 seconds; completing an exercise starts one timer even if it confirms several sets.
- Backlight includes Off, TV Sync, color, brightness, effects and Automatic; bedtime scheduling and TV Sync capture/recovery belong in an advanced view.
- Complete Exercise confirms all displayed sets as performed and advances. Individual sets can be edited or unchecked for deviations. Users can navigate freely between exercises without completing them.
- Do not include weekly goals, progression algorithms or generated training targets. Prefill from actual past workouts only.
- Home has four grouped sections: TV, Speaker, Lights and Backlight, with frequent actions directly accessible. A larger TV remote opens within Home.
- Automatically sync fitness only over Wi-Fi; mobile data leaves changes queued. Show a quiet saved-on-device/waiting-for-Wi-Fi status. Home controls may use any connection that reaches the API.
- First-time setup requires connectivity to validate API access and download existing library, history and profile. Subsequent app launches and fitness logging do not require connectivity or repeated login.

Removing a feature from this app does not authorize deleting its API, Dashboard implementation or stored data.

## Final implementation proposal

No product questions remain from the interview. The published feature spec is the authoritative implementation handoff; the following choices informed it:

- Create a separate native Android project in `mobile/`, using Kotlin and Jetpack Compose, with its own application identity so it can coexist with Expo during validation. The user explicitly selected `mobile/` during spec synthesis.
- Use a persistent local database for fitness reads and writes, an outbox for pending changes and Wi-Fi-constrained background synchronization. Keep active workout drafts local; finishing records a local workout and queues its synchronization. Preserve pending changes across restarts and failed requests.
- Use durable rest-timer deadlines rather than a decrementing counter tied to a composable or screen lifetime. Timer state must survive navigation and process recreation.
- Keep home commands online-only and do not queue them to execute later. Preserve the existing distinction between command acknowledgement and observed device state.
- Reuse existing Axum endpoints where possible; add only backend support necessary for reliable offline synchronization, such as duplicate-safe retries. Do not add multi-client conflict-resolution features.
- Integrate Android build, lint and relevant tests through `devenv shell -- just ...`, adding native-area recipes and a check gate. Run API checks when API code changes.
- Keep Expo operational during validation. Document its deprecation and make native Android the supported client after native acceptance passes; retain existing fitness data and API/Dashboard features.

## Existing domain constraints

The repository's domain language lives in `CONTEXT.md`. A Mobile Client talks only to the Axum API. Backlight is distinct from Lights. A Profile is fitness configuration, not a user account. A Workout is distinct from a NOOP Workout. Existing navigation definitions describe the current client and will need an explicit scope distinction when the new navigation is agreed.

## Initial code findings

- `mobile-expo/src/ui/fitness/workouts-view.tsx` keeps the workout draft in component state and saves only at End Workout. Switching fitness subviews loses the draft. Sets start blank and display reps, weight, duration and RPE together.
- Current mobile fitness subviews are Workouts, Library, Progress and Body. Body combines dated body-weight recording/history with profile settings.
- The current Dashboard LED view is `/backlight`, implemented in `dashboard/src/routes/backlight.tsx` and `dashboard/src/components/backlight-controls.tsx`.
- Existing backlight APIs cover power, TV Sync, RGB, brightness, effects, Automatic, bedtime routine, TV Sync profile capture and backup recovery. Mobile currently has no backlight client.
- The workout API has no first-class active session or durable draft. A client-side session design must explicitly settle persistence and synchronization.

## Design process

Resolve dependent decisions in interview rounds. Record settled terms and decisions as they emerge. Define acceptance criteria before implementation, and obtain confirmation of the complete design before building the app.

## Confirmed acceptance criteria

- Opening a saved workout from History and using Android back returns to History, through both the system gesture and system back button.
- Switching tabs, leaving the workout UI, closing the app or restarting the process does not lose an active workout.
- A workout can be logged and finished without network access; reconnecting on Wi-Fi permits synchronization without duplicate creation on retries.
- The new app launches into Workout, exposes only Workout and Home as bottom tabs, has no Progress tab and uses a dark theme only.
- Repeating a workout preserves exercise order and previous set values without marking any copied set as performed.
- An unfinished workout remains resumable and its queued changes remain pending when Wi-Fi or the API is unavailable.
- Complete Exercise confirms the displayed sets and advances; jumping to another exercise alone never completes any set.
- Finishing with unchecked sets offers a summary and choice to return or omit those sets. Copied, unconfirmed values never silently become recorded performance.
- The rest timer defaults to 150 seconds, starts once per explicit set/exercise completion action and preserves its deadline across navigation and process recreation.
- Fitness changes remain local on mobile data and synchronize when Wi-Fi and the API are available. A retried request cannot create duplicate workouts, exercises or body-weight records.
- Downloaded library/history/profile remain available offline, and local exercise creation, history edits, profile changes and body-weight recording survive restart before sync.
- Repeated workouts preserve prior RPE as well as sets, reps, duration and weight. No progression targets are generated.
- Profile and all Home sections remain reachable through correct in-app back navigation. At navigation roots, follow normal Android back behavior.
- Home commands are never deferred until connectivity returns. Lights and IR command acknowledgement must not be presented as confirmed device power state.

## Verification plan

Write a failing regression test for workout-detail system-back navigation before implementing its fix in the new navigation flow. Add meaningful behavior tests for durable draft recovery, repeated-workout confirmation, exercise completion, omission of unchecked sets, offline edits and duplicate-safe sync after interrupted requests. Verify timer deadlines using a controllable clock. Exercise the finished UI on an Android device or emulator, including process recreation, disconnected logging, Wi-Fi synchronization and dark-only layouts; record any environmental verification limitations explicitly.
