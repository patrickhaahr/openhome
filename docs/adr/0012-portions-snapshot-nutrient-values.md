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

A Portion also copies its NOVA Class when it is logged: its Product's Open Food Facts class,
or its Food's class from `app.db`. The agent classifies a Food the first time it is eaten,
usually just after logging it, so classifying a Food also fills that Food's still-unclassified
Food Portions logged from the same dataset version (a `FoodID` from another version may name
another Food). Already classified Portions keep their class: a changed class applies to
Portions logged afterwards, and a past Meal Log is corrected by replacing its Portions. Product
Portions never take their linked Food's class.

A Supplement Dose copies its Supplement's name, dose unit and per-dose amounts the same way, so
editing or deleting a Supplement changes only later doses; a Meal names its Supplements by ID and
picks up an edited label at its next log.
