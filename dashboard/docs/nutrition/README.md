# Nutrition tracker (#188)

Captures use the production Dashboard and Axum routes with invented data from
`devenv shell -- just dashboard-synthetic`. No household data or credentials
are shown.

| Capture                                   | View                                                                |
| ----------------------------------------- | ------------------------------------------------------------------- |
| [Day, landscape](day-landscape.png)       | Calorie Target, macros and photo Meal Logs at 1280 × 900            |
| [Day, phone](day-phone.png)               | The same Day view at 390 × 844                                      |
| [Meal Logs](day-meals.png)                | Photo, Food, Product and Supplement entries                         |
| [Day, full page](day-full.png)            | Supplements, the micronutrient panel and source credits             |
| [Trends, landscape](trends-landscape.png) | 90 days grouped by ISO week at 1280 × 900                           |
| [Trends, phone](trends-phone.png)         | The same Trends view at 390 × 844                                   |
| [Trends, full page](trends-full.png)      | Intake, weight, NOVA and micronutrient coverage with source credits |

The baseline Dashboard had no Food page. The PR compares the existing Home
capture on `master` with the new Day view.

These desktop Chromium captures do not establish iPad Safari hardware behaviour.
