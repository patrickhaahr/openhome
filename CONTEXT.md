# OpenHome

OpenHome is a homelab control system with an Axum API and mobile clients. Mobile clients interact only with the Axum API; the API owns integration with underlying services and devices.

## Language

### Client and Integration Boundaries

**Mobile Client**:
A user-facing app that consumes the Axum API and does not communicate directly with homelab devices or service bridges.
_Avoid_: Direct bridge client, LAN device client

**CLI Client**:
A noninteractive command-line program that consumes the Axum API and does not communicate directly with homelab devices or service bridges.
_Avoid_: Device script, API wrapper

**Agent**:
An automated operator that invokes a CLI Client or OpenHome tools to fulfill a user request.
_Avoid_: Integration Service, direct device agent

**Axum API**:
The only backend boundary that mobile clients call to read state and trigger actions.
_Avoid_: Gateway, proxy layer

**Integration Service**:
A backend component that knows how to talk to a specific homelab service or device bridge behind the Axum API.
_Avoid_: Client app service, mobile adapter

### Client Access

**Base URL**:
The configured root address of the Axum API that the Mobile Client connects to.
_Avoid_: Device address, bridge address

**API Key**:
A shared bearer credential that authorizes a Mobile Client to call the Axum API.
_Avoid_: User token, session token

**Setup Flow**:
A client-side step where the user enters the Base URL and API Key and validates them against the Axum API before first use.
_Avoid_: Account onboarding, server registration

**Native Client Access**:
The access model for `mobile-expo`: complete Setup Flow once, then open directly into the app.
_Avoid_: Biometric gate, locked-by-default startup

**Home Geofence**:
A user-configured circular region centered on the Mobile Client's current location. When the client exits the region, it asks the Axum API to turn off the lights.
_Avoid_: Continuous GPS tracking, direct light automation

**Home Radius**:
The distance in meters from the center of the Home Geofence that defines when the Mobile Client has left home.
_Avoid_: GPS interval, polling distance

### IR Control

**IR Remote**:
A user-facing control surface that exposes named infrared commands through the Axum API.
_Avoid_: IR device, bridge, blaster

**Command**:
A named IR action that the user triggers from an IR Remote.
_Avoid_: Action, button press, event

### Lights

**Light Command**:
An on or off request sent through the Axum API to the configured SwitchBot
lights. An accepted Light Command acknowledges the request only; it never
establishes the lights' power state.
_Avoid_: Light toggle, IR Command, light state

### Backlight

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

### Mobile Navigation

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

### Dashboard

**Dashboard**:
A user-facing OpenHome interface on a dedicated household display, combining
information and home controls with access to focused feature views.
_Avoid_: Home Tab, Training Context

**Dashboard Home**:
The glanceable overview shown by the Dashboard, where selecting a feature opens
its focused view and inactivity returns to the overview.
_Avoid_: Home Tab, permanent widget grid

**Dashboard Remote**:
A Dashboard control surface for one IR Remote. It shows only the Remote Buttons
whose Commands the Axum API currently reports, omitting rather than disabling
the rest. An accepted Command acknowledges the request only; it never
establishes the device's power, input or volume.
_Avoid_: Unavailable Remote Button, device state panel

**Idle Slideshow**:
A full-screen show of photos from the selected Immich albums that covers the
Dashboard after a configured spell of inactivity, with the time, date and
today's weather and Show When Idle dates on one plate. Since dates count elapsed
time, yearly dates rotate nearest first, and the figures re-read at Copenhagen
midnight. Photos from earlier years on this day lead the slideshow and show their
age quietly on the same plate. The first touch or key only wakes it and returns
to the page and state that were showing; it never reaches a control beneath.
_Avoid_: Screensaver, photo gallery, Photos page

**Idle Block**:
A declared reason the Dashboard must not enter the Idle Slideshow: an open
sheet or dialog, an unsaved draft, a save in flight or a conversation.
_Avoid_: Wake lock, busy flag

**Household User**:
A configured person who owns Wishlists and Watchlists and can be tagged on
Important Dates. The shared Dashboard
shows household members together within these list views, without separate
passwords or private lists.
_Avoid_: Fitness Profile, authenticated account

**Wishlist**:
A Household User's named collection of items they would like to acquire.
_Avoid_: Shopping checklist, Watchlist

**Wishlist Item**:
A desired item recorded in a Wishlist, described by a generic request or a
specific product with an optional link and supporting details.
_Avoid_: Gift reservation, shopping order

**Loop**:
A generated walking or running route that starts and ends near the shared map
origin, measured from its own geometry and compared with the requested
distance. It becomes the Active Route only when the household selects it.
_Avoid_: Walking Loop, Active Route, fabricated route

**Loop Alternatives**:
Up to three Loops for one request that mostly follow different ways, matching
loops first. Fewer are shown, and the shortfall explained, when the search finds
fewer distinct loops within tolerance; none is invented to fill the set.
_Avoid_: Route variants, padded results

**Step Target**:
A Loop target given in steps and converted to distance with the household's
adjustable step length. Step counts shown beside a Loop are estimates from that
step length, never measurements.
_Avoid_: Step tracking, measured steps

**Active Route**:
The selected walking or running route made available to the Dashboard and the
Mobile Client for viewing with the phone's current location.
_Avoid_: GPX transfer, route screenshot

**Route Session**:
A started walk or run on the Mobile Client that keeps its chosen route until
the user accepts a replacement or ends the session.
_Avoid_: Workout, NOOP Workout

**Watchlist**:
A Household User's collection of movies and series planned for watching,
currently being watched or completed, with dates recording addition and completion.
_Avoid_: Wishlist, viewing history import

**Watchlist Entry**:
One movie or whole series on a Household User's Watchlist, with a planned,
watching or completed status and addition/completion dates.
_Avoid_: Episode log, Wishlist Item

**Important Date**:
A household-shared date the household counts from or towards, with a title and
a kind (Since or Yearly). It is never owned: it tags the Household Users it
concerns, both members for a shared anniversary and none for a friend's
birthday. Axum computes its figures for a Europe/Copenhagen day.
_Avoid_: Reminder, calendar event, owned date

**Since**:
An Important Date kind that counts up from the date: whole years, months and
days elapsed, plus the total in days.
_Avoid_: Countdown, age

**Yearly**:
An Important Date kind that counts down to the date's next anniversary. In a
common year, 29 February falls back to 28 February.
_Avoid_: Recurring reminder, repeat rule

**Milestone**:
An Axum-computed celebration on an Important Date's Copenhagen day: a Since
anniversary, 100 or 500 days, each thousand days, or a Yearly occurrence.
Anniversaries take precedence over round day counts. Only dates with Show When
Idle enabled may supply the idle screen's leading Milestone.
_Avoid_: Reminder, client-computed celebration, notification

**Show When Idle**:
An Important Date's switch allowing the shared idle photo screen, which anyone
in the room can see, to show it. Off by default.
_Avoid_: Private date, pinned date

**Shared Viewing Progress**:
Viewing state reported by the household's shared Jellyfin account, without
attributing it to an individual Household User.
_Avoid_: Personal completion history, Watchlist status

### Server Operations

**Server Tab**:
A mixed Top-Level Tab on `mobile-expo` that groups server-operations surfaces: AdGuard Protection control, the Docker Health Summary, and the Compact Timeline with Feed management.
_Avoid_: Infra dashboard, ops console

**Docker Tab**:
A Top-Level Tab on `mobile-expo` that lists Containers with their run state and exposes log inspection and lifecycle actions.
_Avoid_: Container manager, DevOps screen

**Container**:
One Docker workload reported by the Axum API with name, image, run state, and health status.
_Avoid_: Service, pod, stack

**Docker Health Summary**:
The client-side classification of all Containers into healthy, unhealthy, idle (none running), or offline (Axum API unreachable).
_Avoid_: Cluster status, uptime metric

**AdGuard Protection**:
The network-level ad blocking managed through the Axum API and controlled from the Server Tab.
_Avoid_: DNS filter, firewall setting

**Protection Pause**:
A time-boxed suspension of AdGuard Protection that resumes automatically when it ends.
_Avoid_: Disable, timeout

**Feed**:
A subscribed RSS source whose items appear in the Compact Timeline.
_Avoid_: Channel, subscription URL list

**Compact Timeline**:
The compact view of Feed items served by the Axum API that the Mobile Client paginates through newest-first.
_Avoid_: Article reader, full-content feed

### Fitness

**Exercise**:
A named movement in the exercise library, with muscle group and equipment metadata, referenced by logged workout entries.
_Avoid_: Movement item, activity

**Exercise Category**:
A library tag on an Exercise (`calisthenics` or `gym`) that organizes the exercise library; it is descriptive metadata, not a rule about where the exercise may appear.
_Avoid_: Exercise type, workout type

**Workout**:
A logged training session on a specific date, containing ordered exercise entries with sets.
_Avoid_: Training plan, session template

**NOOP Workout**:
An activity record in NOOP with start and end timestamps and a recorded source, distinct from a Workout logged in OpenHome.
_Avoid_: Automatically matched Workout, detected-only Workout

**NOOP Source**:
A logical origin of NOOP observations. A strap source and its on-device computed sibling can have different identifiers while representing one physical strap.
_Avoid_: Second strap, second person

**NOOP Workout Detail**:
One NOOP Workout's strap heart rate series, time in each Heart Rate Zone, Heart Rate Recovery and average pace.
_Avoid_: Workout details, full workout dump

**Heart Rate Recovery**:
The drop from the highest heart rate in a NOOP Workout's final 30 seconds to the heart rate 1, 2 and 5 minutes after it ends, computed as the NOOP app does.
_Avoid_: HRR peak, recovery score

**Training Day**:
A calendar day used to compare logged training with NOOP observations. NOOP daily results keep their recorded day; timestamped observations are grouped in the user's home time zone (Europe/Copenhagen).
_Avoid_: UTC day, rolling 24-hour period

**Sleep Night**:
A calendar night identified by the local day on which sleep ends. A Sleep Night remains present in a recent-night series when no sleep result is available.
_Avoid_: Last available sleep record, sleep start date

**Recovery Day**:
One Training Day's sleep and recovery read from NOOP: the Sleep Night that wakes on that day, plus resting heart rate, HRV, skin temperature deviation, respiratory rate, the day's Journal Entries, and NOOP's derived recovery and strain scores. Each value carries its unit and the NOOP Source it was selected from. The Recovery Day also states the sync freshness and coverage, so a day that has not synced yet is not mistaken for a recorded absence.
_Avoid_: Recovery score, readiness, last night's data

**Training Context**:
A compact, date-aligned view of up to 90 Training Days for a programming discussion. Each day summarizes its logged Workouts per Exercise entry, the NOOP Workouts that start on it, Body Weight, and the Recovery Day's sleep duration, resting heart rate, and HRV. Freshness and coverage are given per NOOP read. Exact Sets and sleep sessions stay in the focused reads.
_Avoid_: Training log dump, dashboard, combined workout

**Mixed-Category Session**:
A Workout that contains entries from both Exercise Categories (calisthenics and gym); nothing restricts a Workout to a single category.
_Avoid_: Single-category workout, category-specific session

**Set**:
One performance of an Exercise within a Workout entry, with reps, optional added weight, or a hold duration.
_Avoid_: Rep block, attempt

**Added Weight**:
The external weight in kg on a Set, on top of bodyweight: a bodyweight movement stores `weight_kg` null, not zero; a 40 kg weighted pull-up stores 40.
_Avoid_: Total working weight, bodyweight-inclusive load

**Timed Hold**:
A duration-only Set that stores `duration_seconds` with `reps` null (e.g. Planche Hold); a Set must have reps or a duration, never neither.
_Avoid_: Rep-based hold, timed rep set

**Volume**:
A Set's contribution computed as `reps * weight_kg`, or `duration_seconds * weight_kg` when both are present; Sets without Added Weight contribute zero.
_Avoid_: Tonnage including bodyweight, rep count

**Progress Series**:
The per-date aggregate of one Exercise's logged Sets served by the Axum API: best reps, best added weight, total Volume, best RPE, and the Epley estimated 1RM from the best set of the day; observed data only, no interpolation.
_Avoid_: Interpolated trend, forecast chart

**Progress Line**:
One Exercise's observed progress indexed for a combined chart: best unweighted reps, longest unweighted Timed Hold, or best eligible Epley added-load estimate on each logged date, divided by its baseline (the first date in the period with a value above zero) and multiplied by 100. Weighted reps use positive Added Weight at 1–10 reps; one rep uses the lifted load. Raw values, units and omitted dates stay beside the index. An Exercise that mixes reps with holds, has weighted holds, or has no valid estimate has no line. Estimated kilograms exclude Body Weight; weighted bodyweight movements show an added-load proxy, never total strength.
_Avoid_: Strength score, combined volume, trend line

**Body Weight**:
A user-recorded day weight in kg, one row per date and kept only in the daily body-weight record, separately from Workouts.
_Avoid_: Workout body_weight, weight log entry

**Profile**:
The single user configuration row holding height, sex, birth date and Max Heart Rate used to interpret fitness and nutrition data.
_Avoid_: User account, settings record

**Max Heart Rate**:
The user-set heart rate ceiling on the Profile that defines Heart Rate Zones; it is never inferred from recorded data.
_Avoid_: HRmax ever, observed max, age-predicted max

**Heart Rate Zone**:
One of five bands at 50, 60, 70, 80 and 90–100 % of Max Heart Rate, matching the NOOP app's default zones; time below 50 % is outside every zone.
_Avoid_: Custom zone, heart-rate-reserve zone

**Journal Entry**:
A user's answer to one NOOP journal question for the day it was logged against in NOOP.
_Avoid_: Survey, habit log

### Nutrition

**Food**:
One entry of the Danish Food Composition Database (formerly Frida) whose energy,
macronutrients and micronutrients are given per 100 g edible part. Mixed dishes
are logged as their Foods, never as a user-made food.
_Avoid_: Ingredient, Custom Food, item

**Product**:
A packaged food identified by its barcode, linked to the one Food that best
matches it. Its label supplies energy and macronutrients; the linked Food
supplies vitamins and minerals.
_Avoid_: Branded Food, barcode item

**NOVA Class**:
The processing class (1–4) of a Food or Product, decided once and kept with it.
Products take Open Food Facts' class; unclassified items are left out of NOVA
shares rather than guessed.
_Avoid_: Processing score, food quality score

**Portion**:
A Food or Product with an eaten amount in grams; its nutrients are the per-100 g
values scaled to that amount. Grams may be the user's estimate.
_Avoid_: Serving, food entry

**Meal**:
A named, reusable set of Portions that can be logged again without analysing a
photo. Editing a Meal never changes Meal Logs made from it.
_Avoid_: Meal template, recipe, Meal Log

**Meal Log**:
One eating occasion at an explicit Copenhagen time, holding its own copy of
Portions and an optional photo. It counts toward the calendar day of that time,
which may be in the past when the user says so.
_Avoid_: Meal, food diary entry, check-in

**Recommended Intake**:
The NNR 2023 daily amount of a nutrient for the Profile's sex and age band,
either a recommended or an adequate intake. Reaching or exceeding it is good.
_Avoid_: RDA, goal, daily value

**Upper Limit**:
EFSA's Tolerable Upper Intake Level: the highest habitual daily intake unlikely
to cause harm. Some apply only to the supplement share of a nutrient. A 7-day
average above it means cut down; a single day above it is only a caution.
_Avoid_: Overdose threshold, maximum, red line

**Supplement**:
A labelled product whose amounts are given per dose (tablet, capsule, scoop or
ml) rather than per 100 g. It may supply vitamins and minerals or other
compounds such as creatine or omega-3 fatty acids.
_Avoid_: Pill, Food, medication

**Supplement Dose**:
A Supplement with a number of doses taken, logged at a time like a Portion and
able to belong to a Meal.
_Avoid_: Pill log, intake

**Incomplete Day**:
A day the user has said was not fully logged. Its intake is left out of the
Estimated Expenditure until the user says the day is complete; backfilling a
meal does not clear it.
_Avoid_: Missed day, partial day

**Trend Weight**:
Body Weight smoothed over time so daily water and salt swings do not read as
fat gain or loss.
_Avoid_: Average weight, scale weight

**Estimated Expenditure**:
The energy the user burns per day, inferred from logged intake and the change in
Trend Weight over the last 28 days, starting from a formula estimate until enough
days are logged. Exercise is already contained in it.
_Avoid_: Maintenance, TDEE, BMR, burned calories

**Nutrition Goal**:
The user's chosen direction and weekly rate: lose, maintain or lean bulk. A
change takes effect from the day it is set.
_Avoid_: Diet, phase, deficit goal

**Calorie Target**:
The daily energy to eat: Estimated Expenditure adjusted by the Nutrition Goal's
rate, set each Copenhagen Monday and held for that week, never below BMR. The
same target applies on training and rest days.
_Avoid_: Maintenance, budget, goal

**Macro Targets**:
The day's protein target of 2.2 g per kg Trend Weight, a fat floor, and carbs as
whatever energy of the Calorie Target remains.
_Avoid_: Macro split, macro ratios

## Relationships

- A **Mobile Client** calls the **Axum API** for every user-visible capability.
- A **CLI Client** calls the **Axum API** for all device and service access.
- An **Agent** invokes a **CLI Client** or OpenHome tools to fulfill a user request; training tools remain read-only.
- The **Axum API** delegates device-specific work to one or more **Integration Services**.
- A **Mobile Client** uses a configured **Base URL** to reach the **Axum API**.
- A **Mobile Client** authorizes requests to the **Axum API** with an **API Key**.
- A user completes the **Setup Flow** before first use of the **Mobile Client**.
- `mobile-expo` can register one **Home Geofence** and sends a light-off command to the **Axum API** when it exits that region.
- A **Home Radius** defines the boundary of the **Home Geofence** in meters.
- `mobile-expo` uses **Native Client Access**.
- An **IR Remote** exposes one or more **Commands**.
- A **Mobile Client** triggers **IR Remote** **Commands** by calling the **Axum API**.
- The **Dashboard** sends **Light Commands** through the **Axum API** and shows the lights' observed power as unknown.
- The **Backlight** is controlled separately from the wall-switch **Light Commands**.
- **TV Sync** is a **Backlight** mode; it does not establish the TV's power state.
- A **Backlight Override** temporarily replaces the **Backlight Routine**.
- **Bedtime Mode** supplies the red window within the **Backlight Routine**.
- A **Home Tab** combines summaries and shortcuts from multiple **Top-Level Tabs**.
- Each **Top-Level Tab** intentionally aligns to one Axum API area.
- A **Remote Button** maps to exactly one **Command**.
- The Android client owns the **Remote Button Set** for the single **IR Remote**.
- The **V1 Remote Button Set** is a fixed client-defined subset of IR Commands.
- An **Unavailable Remote Button** stays visible but disabled when its **Command** is absent from `available_commands`.
- The **Home Tab** preloads IR state for both **Home Remote Controls** and the full **IR Remote** tab.
- **Home Remote Controls** are a subset of the **Remote Button Set**.
- The **V1 Home Remote Controls** are `bluetooth` and `optical`.
- The **Server Tab** groups **AdGuard Protection** control, the **Docker Health Summary**, and the **Compact Timeline**.
- A **Protection Pause** leaves AdGuard Protection off only until its duration ends.
- A **Container** is inspected (logs) and lifecycle-managed (start, stop, restart) through the Axum API from the **Docker Tab**.
- Each **Feed** contributes items to the **Compact Timeline**.
- A **Workout** references Exercises from the exercise library and records their Sets.
- A **NOOP Workout** remains distinct from a logged **Workout**.
- A strap **NOOP Source** and its computed sibling may contribute observations for the same person and day.
- A **Training Day** aligns logged training and NOOP observations without changing a NOOP daily result's recorded day.
- A **Training Day** shows its logged **Workouts** beside the **NOOP Workouts** that start on it locally; a NOOP Workout on the same day is context, never an asserted match.
- A **Sleep Night** is indexed by wake day, including nights without a result.
- A **Training Context** keeps one entry per **Training Day**, including days with nothing logged or synced, and never pairs a logged **Workout** with a **NOOP Workout**.
- A **Recovery Day** reports the **Sleep Night** that wakes on it and resolves each value from a strap **NOOP Source** first, falling back to its computed sibling.
- A **Mixed-Category Session** may combine calisthenics and gym entries in one Workout.
- An **Added Weight** of null means bodyweight; Volume treats it as zero.
- A **Timed Hold** replaces reps with a duration.
- A **Progress Series** aggregates logged Sets per date and never invents data between dates.
- A **Progress Line** has samples only on logged dates; a never-logged or unsupported Exercise gets no line, never a zero or flat one.
- **Body Weight** is recorded per date by the client and is not stored on a Workout.
- The **Profile** is configured by the client and consulted to interpret fitness data.
- A **Heart Rate Zone** is always relative to the current **Max Heart Rate**; changing it re-zones all past heart rate data.
- Heart rate for any **NOOP Workout** comes from the strap **NOOP Source**, whatever source recorded the NOOP Workout.
- A **Journal Entry** keeps the day it was logged against; it is never shifted to the night or day it describes, and it appears on the Recovery Day for that same day.

## Example dialogue

> **Dev:** "Should the Android app call the IR bridge directly when it is on the same LAN?"
> **Domain expert:** "No. The **Mobile Client** only talks to the **Axum API**. Direct bridge communication is not part of the client model."

## Flagged ambiguities

- "feature tab" and "API endpoint category" were both used for top-level navigation. Resolved: the app uses **Top-Level Tabs** that intentionally mirror Axum API areas, while **Home Tab** is the mixed overview.
- The IR API may expose more commands than the Android app shows. Resolved: the Android app uses a curated **Remote Button Set**, and the `test` command is not exposed.
- "command" was used for both IR actions and light on/off requests. Resolved: an IR action is a **Command**; a light request is a **Light Command**.
- "test endpoint" was used to refer to IR behavior. Resolved: the current API has a `test` **Command**, not a separate endpoint.
- "Top-Level Tabs mirror one API area" conflicts with the Server Tab grouping three areas. Resolved: the **Server Tab** is a deliberate mixed operations tab, playing the same role for server ops that the **Home Tab** plays for quick access.
