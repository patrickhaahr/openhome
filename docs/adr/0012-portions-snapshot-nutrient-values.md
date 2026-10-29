# Portions snapshot nutrient values

A logged Portion stores its own copy of every per-100 g parameter it was logged
with (all 232 database parameters, label values first for a Product), instead of
reading the live Food or Product. The Danish Food
Composition Database renumbers and replaces Food IDs between versions (6.1
replaced 5.5 and dropped the Frida name), and Products can be re-linked to a
better-matching Food; a live reference would silently rewrite past days, past
Calorie Targets' adherence and the Estimated Expenditure derived from them.

## Consequences

Correcting a Product's linked Food or upgrading the database affects only
future Portions. Past Meal Logs are corrected one at a time by replacing their
Portions. Meals keep references, not values, so the next log of a Meal picks up
the corrected data.
