# Weekly target record fixed on first read

The Calorie Target is set each Copenhagen Monday from that week's Estimated
Expenditure and held all week, so a backfilled meal or weigh-in must never
change a week's target afterwards. OpenHome has no scheduler for nutrition, so
the API stores each week's Estimated Expenditure and BMR (`nutrition_weeks`) on
the first read of any day in that week, computed only from Meal Logs and Body
Weight dated before its Monday, and never updates the row. Past weeks that were
never read before are fixed the same way when first viewed, from the data as it
is then. Weeks starting after today are computed for display and not stored.

The week's BMR uses the Trend Weight on the Sunday before it; when the first
weigh-in falls inside the week, it uses that weigh-in, and days before it have
no targets. The Profile is read when the record is written, so a stored week
keeps its target after Profile edits. The goal is not stored with the week: the
Calorie Target applies the Nutrition Goal in effect on each day to the week's
stored expenditure, so a mid-week change applies from its day, and earlier days
keep the goal they had.

## Considered Options

- A Monday job: needs a scheduler and misses Mondays while the API is down;
  a missed week would still need the lazy path.
- Computing every week live from all data: backfills rewrite past targets.
- Storing a Calorie Target per day: a goal change would need to rewrite the
  rest of the week's rows.

## Consequences

A week's target depends on when it is first read: a week first viewed after
a backfill includes it. Macro Targets follow the day's Trend Weight and are not
stored, so a backfilled weigh-in changes past days' protein and fat targets,
never their Calorie Target. Deleting a week's row is the only way to recompute
it.
