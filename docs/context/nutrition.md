# Nutrition

Part of the OpenHome domain language; see [CONTEXT.md](../../CONTEXT.md).

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
average above it means cut down; a single day above it is only a caution. The
average covers the days with Meal Logs among the last seven, so untracked days
never dilute it, and says cut down only when at least 3 of those days know the
amount: one or two days above the limit are a caution, not a habit.
_Avoid_: Overdose threshold, maximum, red line

**Lasting Shortfall**:
A nutrient whose mean over a range's logged days stays below 70 % of its
Recommended Intake, over at least 7 logged days with a known amount. A low day
or a short stretch of logs is never one, and unknown amounts are never counted
as short.
_Avoid_: Deficiency, deficit, low day

**Supplement**:
A labelled product whose amounts are given per dose (tablet, capsule, scoop or
ml) rather than per 100 g. It may supply vitamins and minerals or other
compounds such as creatine or omega-3 fatty acids.
_Avoid_: Pill, Food, medication

**Supplement Dose**:
A Supplement with a number of doses taken, logged at a time like a Portion and
able to belong to a Meal.
_Avoid_: Pill log, intake

**Trend Weight**:
Body Weight smoothed over time so daily water and salt swings do not read as
fat gain or loss. It starts at the first weigh-in, moves a tenth of the way to
each later weigh-in, and holds on days without one.
_Avoid_: Average weight, scale weight

**Estimated Expenditure**:
The energy the user burns per day, inferred from logged intake and the change in
Trend Weight over the last 28 days, starting from a formula estimate until enough
days are logged. Days without Meal Logs are left out; backfilled meals count once
logged. Exercise is already contained in it.
_Avoid_: Maintenance, TDEE, BMR, burned calories

**Nutrition Goal**:
The user's chosen direction and weekly rate: lose, maintain or lean bulk. A
change takes effect from the day it is set.
_Avoid_: Diet, phase, deficit goal

**Calorie Target**:
The daily energy to eat: Estimated Expenditure adjusted by the Nutrition Goal's
rate, set each Copenhagen Monday and held for that week, never below BMR. The
same target applies on training and rest days. A week's Estimated Expenditure is
fixed the first time any of its days is read, from the data before its Monday.
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
