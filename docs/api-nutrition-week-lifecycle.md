# Nutrition week lifecycle

Disposition for [#248](https://github.com/patrickhaahr/openhome/issues/248),
under #241/#240. [ADR 0014](adr/0014-weekly-target-record-fixed-on-first-read.md)
remains the policy. `nutrition::targets` owns eligibility, computation and
freezing; no scheduler, persisted status or recomputation path is added.

## Authoritative boundary

| Existing row / computation | Read behavior | Persistence |
| --- | --- | --- |
| Row present | Reuse frozen expenditure, BMR and basis | Never update |
| No row; Profile or computation prerequisites missing | No week value is invented | No insert |
| No row; computable week starts after today's ISO Monday | Display provisional computation (`week_fixed: false`) | No insert |
| No row; computable week starts on/before today's ISO Monday | Freeze, then use the stored winner (`week_fixed: true`) | Insert unless another reader won |

`Week::fixed` is an existing in-memory/API provenance flag. `Week::compute`
sets it false; `stored_weeks` sets it true when decoding a row. Database row
presence remains the frozen-state model, so another lifecycle enum or column
would duplicate it. The renamed `freeze_weeks` owns insertion **and** rereading
only the requested records; callers receive database values, never an eligible
computation relabelled as fixed.

## Calendar and first read

`plan` groups requested days by ISO Monday and remembers the latest **requested**
day in each week. It loads weigh-ins through the range's `to`, goals, stored
weeks and Profile in the existing order. Unstored weeks alone are computed.
Logged energy is loaded once for their combined window; each computation uses
only the 28 calendar days before its own Monday.

BMR uses Trend Weight on the preceding Sunday, carried from the most recent
weigh-in. With no earlier weight, it uses the first weigh-in from Monday through
that week's latest requested day. That bound remains the requested day, without
capping it at today or extending it to Sunday. A multi-day Summary can therefore
freeze a week using a weigh-in later than its earliest requested day; days before
that weigh-in still lack daily Trend Weight and remain unavailable.

Eligibility compares week Mondays, not individual requested days. Reading a
future day in the current week may freeze that week. A wholly future week stays
provisional and is recomputed on later reads; its first computable read after
becoming eligible freezes it. An unread past week uses the data available when
it is first computed, including backfills already present then.

The route's `today()` uses `CalendarDay::of(Utc::now())` in Europe/Copenhagen.
The planner uses that supplied calendar day throughout; it adds no clock read.
ISO week calculation, including year transitions, stays in `CalendarDay`.

## Concurrent winner and daily projection

`freeze_weeks` inserts eligible records in Monday order in one transaction,
using the unchanged `ON CONFLICT (monday) DO NOTHING`. It commits before rereading
the stored range, then requires each requested record to be present. A
concurrent insert's stored values win; request start order does not establish
the winner. The input reads are not a single database snapshot, and this change
adds neither a wider transaction nor new locking/retry behavior.

Backfilled intake, weigh-ins and Profile edits never update the frozen record.
An existing row can supply a day's targets even when the current Profile is
incomplete, but the day still needs Trend Weight on/before it. Without a week,
missing Profile fields are reported; absent daily Trend Weight adds Body Weight.
Other noncomputable cases retain the existing unavailable projection.

Daily `goals.on(day)` and daily Trend Weight remain independent inputs to
`DayPlan::new`: goal changes can change the Calorie Target against frozen
expenditure/BMR, while weigh-in changes can change Macro Targets. Freezing the
week does not freeze the entire daily response.

Nutrition Day calls `plan_day`, which delegates to `plan`; Nutrition Summary
calls `plan` directly. Summary rejects ranges ending after today, while Day
planning has the future-week behavior above. Both reads may freeze computable
unstored weeks through the same path. SQL text, schema and SQLx metadata are
unchanged.

## Validation

`nutrition_targets` proves each transition through the public Nutrition Day and
Summary routes: the provisional future week invents a value but stores no row,
while every reading of the current week freezes its record
(`the_current_week_freezes_on_first_read_and_future_weeks_stay_provisional`);
the first in-week weigh-in reaches only the read whose latest requested day does,
so a Day read of the week's Monday rests on nothing and a Summary over the whole
week freezes it, with the days before that weigh-in staying target-less in the
frozen week
(`the_first_weigh_in_bound_is_the_latest_requested_day_not_the_whole_week`);
a stored week keeps its targets once the Profile has lost every input a new week
would need, which stays unavailable without guessed values
(`a_stored_week_reads_without_the_profile_and_future_weeks_stay_unavailable`);
a first-read commit that landed ahead of a read's freeze wins unchanged against
what the live data now computes
(`the_stored_row_wins_over_the_computation_and_later_backfills`); racing first
reads return one stored winner with one row
(`concurrent_first_reads_agree_on_the_stored_winner`); and the ISO week across
New Year is one Monday-to-Sunday record
(`new_years_week_stays_one_monday_to_sunday_record`). Backfilled intake and
weigh-ins, the goal of each day and the existing missing-input, blend, BMR-floor
and Summary-freezing coverage stay in their earlier tests.

All of these compare against the live Copenhagen calendar, where today decides
which weeks have started: they pin both sides of the future-to-current boundary,
while a named future week's own handover to frozen happens when its week begins.
The temporary-file concurrent test and the standard `:memory:` fixtures run
through `devenv shell --` with `SQLX_OFFLINE=true`; no schema, SQL metadata or
seed data outside `nutrition_weeks` is written by the tests.
