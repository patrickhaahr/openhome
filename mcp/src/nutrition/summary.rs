use rmcp::{handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router};
use serde_json::json;

use super::{NutritionSummaryRequest, SetNutritionGoalRequest};
use crate::OpenHomeTools;

const NUTRITION_SUMMARY: &str = "Every nutrition trend over Europe/Copenhagen days from `from` through `to` (1 to 365 \
days, ending today at the latest), grouped by aggregation: day, week (ISO, Monday to Sunday) or month (calendar). The \
first and last week or month cover only their days inside the range and say partial=true. Returns logged_days and \
missing_days for the range and `periods` in date order, each with start, end, days, partial, logged_days (days with \
Meal Logs, Supplement Doses alone included), missing_days (no Meal Logs; never counted as zero) and intake_days \
(logged days whose energy is known; a day of Supplement Doses without energy is never 0 kcal); energy {eaten_kcal \
(mean per intake day), calorie_target_kcal, estimated_expenditure_kcal (means of each day's value)}; protein {eaten_g, \
target_g}; protein and macro_energy {protein_kcal, carbohydrate_kcal, fat_kcal, alcohol_kcal} each average over \
the logged days that know that metric, independently of energy, at 4/4/9/7 kcal per gram. known_days gives \
energy, protein, carbohydrate, fat and alcohol's separate averaged-day counts; nova {nova_1_share, nova_1_kcal, classified_kcal, unclassified_portions} summed over the period; \
weight {trend_weight_kg on the last day, scale_weight_kg (mean of the period's weigh-ins), weigh_ins}; and \
weight_change {kg_per_week, the Trend Weight's change over the period (or the 7 days ending on its last day when \
shorter) per week; goal_kg_per_week, the Nutrition Goal's change, negative to lose}; and micronutrients {logged_days, \
nutrients [{key, percent_of_recommended, averaged_days, days_above_upper_limit, seven_day_average_above_upper_limit}]} \
(null when the range has none). The range's `micronutrients` (state available: logged_days, shortfall {below_percent, \
min_logged_days}, nutrients with name, group, unit, adequate_intake and lasting_shortfall, and sources; state \
unavailable: missing [sex, birth_date] and a reason, also below 18 years) covers every vitamin and mineral of \
nutrition_day's panel plus epa_dha: percent_of_recommended is the mean of each logged day's percentage of the NNR 2023 \
Recommended Intake, leaving out days without Meal Logs and unknown amounts; days_above_upper_limit counts days above an \
EFSA Upper Limit, judged on the same share as nutrition_day (supplements only for magnesium, folic acid and the niacin \
forms); seven_day_average_above_upper_limit is true when a trailing 7-day average ending on a day with Meal Logs was \
above one (cut down); both are null for a nutrient without an Upper Limit. lasting_shortfall is true when the range's \
mean is below shortfall.below_percent over at least shortfall.min_logged_days logged days. Null means unknown: no \
logged day, no target (Profile or Body Weight missing; see nutrition_day for the reason) or no Trend Weight. sources \
lists the Danish Food Composition Database versions the range's values come from and label_source is Open Food Facts \
when a Product was eaten (else null): credit each wherever values are shown. Works while the Food database is \
unavailable.";

const NUTRITION_GOAL: &str = "The user's Nutrition Goal in effect today (direction lose, maintain or lean_bulk with \
rate_kg_per_week, and effective_from, the Europe/Copenhagen day it was set, null for the default of losing 0.5 kg per \
week) and its history of changes, oldest first.";

const SET_NUTRITION_GOAL: &str = "Set the user's Nutrition Goal from today (Europe/Copenhagen), only when the user asks: \
direction lose with rate_kg_per_week 0.25, 0.5 or 0.75; maintain (no rate); or lean_bulk with 0.1 or 0.25. Earlier days \
keep the goal they had, and a second change today replaces the first. Calorie Targets from today use the week's fixed \
Estimated Expenditure plus the rate x 7,700 kcal / 7, never below BMR. Returns the goal and its history like \
nutrition_goal; any other rate is a 400 error.";

#[tool_router(router = nutrition_summary_router, vis = "pub(crate)")]
impl OpenHomeTools {
    #[tool(
        description = NUTRITION_SUMMARY,
        annotations(
            title = "Nutrition Summary",
            read_only_hint = true,
            open_world_hint = false
        )
    )]
    async fn nutrition_summary(
        &self,
        Parameters(NutritionSummaryRequest {
            from,
            to,
            aggregation,
        }): Parameters<NutritionSummaryRequest>,
    ) -> CallToolResult {
        let query = [
            ("from".to_string(), from),
            ("to".to_string(), to),
            ("aggregation".to_string(), aggregation.as_str().to_string()),
        ];
        self.api
            .request(
                reqwest::Method::GET,
                &["api", "nutrition", "summary"],
                None,
                &query,
            )
            .await
    }

    #[tool(
        description = NUTRITION_GOAL,
        annotations(title = "Nutrition Goal", read_only_hint = true, open_world_hint = false)
    )]
    async fn nutrition_goal(&self) -> CallToolResult {
        self.api.get(&["api", "nutrition", "goal"]).await
    }

    #[tool(
        description = SET_NUTRITION_GOAL,
        annotations(
            title = "Set the Nutrition Goal",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn set_nutrition_goal(
        &self,
        Parameters(request): Parameters<SetNutritionGoalRequest>,
    ) -> CallToolResult {
        self.api
            .request(
                reqwest::Method::PUT,
                &["api", "nutrition", "goal"],
                Some(json!(request)),
                &[],
            )
            .await
    }
}
