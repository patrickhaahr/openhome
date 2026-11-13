# Fitness

Part of the OpenHome domain language; see [CONTEXT.md](../../CONTEXT.md).

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
