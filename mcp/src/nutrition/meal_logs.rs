use rmcp::{handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router};
use serde_json::json;

use super::{
    LogMealRequest, MealLogRequest, MealLogsRequest, NutritionDayRequest, UpdateMealLogRequest,
};
use crate::OpenHomeTools;

const LOG_MEAL: &str = "Log one eating occasion as a Meal Log: either up to 50 Portions, each a food_id from \
search_foods or the barcode of a Product linked with link_product, with edible grams and optional estimates per 100 g for \
values no source has, and/or up to 50 supplement_doses (supplement_id from supplements and doses), at least one in all, or \
a saved Meal's name in meal (any case; no photo analysis needed) with optional overrides giving one-off grams to items \
named by their food_id or barcode, or one-off doses to Supplements named by supplement_id, at an explicit eaten_at (RFC 3339 with an offset: the send time or the time the user \
describes; never in the future), with an optional note and an optional photo (the user's JPEG or PNG as base64, at most \
10 MB, stored without metadata as AVIF with a thumbnail). It counts toward the Europe/Copenhagen calendar day of eaten_at, \
so past times backfill earlier days. Each Portion copies every per-100 g value (a Product's label values first, then its \
linked Food's, then estimates) and each Supplement Dose copies its label's per-dose amounts, so later database updates, \
re-links, Supplement edits and Meal edits never change it. Returns meal_log (id, \
eaten_at in Copenhagen time, day, meal_name (the Meal it was logged from, or null), note, portions with food_id, product (barcode, name, brand) or null, names, grams, \
dataset_version, nova_class (the Product's or Food's, null while unclassified) and key nutrients energy_kcal, protein, available_carbohydrate, fat, dietary_fibre and alcohol as {value, \
unit, source}, supplement_doses (supplement_id, name, dose_unit, doses, amounts and compounds taken: per dose times doses), \
totals as {value, unit, unknown_portions, food, supplements}, and photo with its size or null) and replayed. Generate a \
unique request_id per intended meal and reuse identical arguments and ID after an uncertain response: an accepted retry returns replayed=true, changed \
arguments conflict (409) and a deleted Meal Log is not recreated. A null value is unknown, never zero. An unknown food_id \
, unlinked barcode or unknown supplement_id is a 404 error, as is an unknown Meal name; an override naming no item of the Meal is a 400 error; a \
Meal naming a Food the current Food database no longer has is a 409 error with code meal_needs_update (update_meal it); \
a Product whose linked Food the current Food database no longer has is a 409 error with code product_needs_relink \
(link_product it again); \
an estimate for a value the label or Food has is a 400 error; while the Food \
database is unavailable the result is a 503 error with code food_database_unavailable. A photo that is not a JPEG or PNG is a 400 error and one over 10 MB or 12000 px a side is a 413 error; nothing is logged \
then.";

const MEAL_LOGS: &str = "List Meal Logs for one Europe/Copenhagen calendar day (`day`) or an inclusive range of up \
to 90 days (`from` and `to`), oldest first. Each has its id, eaten_at in Copenhagen time with its offset, day, \
meal_name (the saved Meal it was logged from, or null), note, portions (food_id, Danish and English name, grams, dataset_version, nova_class and key nutrients as {value, unit, \
source}), supplement_doses (supplement_id, name, dose_unit, doses, amounts and compounds taken) and \
totals as {value, unit, unknown_portions, food, supplements}: food sums the Portions that have a value, unknown_portions \
counts those that do not, supplements sums the doses whose label lists it, and value adds both. \
A null value is unknown, never zero. Works while the Food database is unavailable.";

const UPDATE_MEAL_LOG: &str = "Correct one Meal Log by meal_log_id. Supplied fields change together; omitted fields \
are preserved. eaten_at moves it to that time's Europe/Copenhagen day; note null clears the note; portions replaces \
every Portion (food_id or a linked Product's barcode, edible grams and optional estimates) with fresh values from the \
current Products and Food database; supplement_doses replaces every dose with the current Supplements' labels (at least one \
Portion or dose must remain); photo (base64 JPEG or PNG, at \
most 10 MB) replaces the photo and photo null removes it. Returns the updated Meal Log. A rejected photo changes nothing. \
A missing Meal Log, food_id or supplement_id is a 404 error; replacing Portions while the Food database is unavailable is \
a 503 error. After an uncertain response, list meal_logs for the day before retrying.";

const DELETE_MEAL_LOG: &str = "Permanently delete one Meal Log by meal_log_id, removing it from its Europe/Copenhagen \
day's totals. Returns deleted=true only after the API confirms removal; a missing Meal Log is a 404 error. An old \
log_meal retry with its request_id cannot recreate it. After an uncertain response, list meal_logs before retrying.";

const NUTRITION_DAY: &str = "One Europe/Copenhagen calendar day of nutrition: day_start and day_end (a day can last 23 \
or 25 hours), `totals` for energy_kcal, protein, available_carbohydrate, fat, dietary_fibre and alcohol, `targets` \
(state available: trend_weight_kg, bmr_kcal, estimated_expenditure_kcal with its expenditure_basis, the goal in effect, \
calorie_target_kcal (the week's expenditure plus the goal's rate, never below BMR), eaten_kcal, remaining_kcal and \
macros: protein target_g with status met or below, fat floor_g with status (below means under the floor) and carbs \
target_g, never graded; state unavailable: missing [height, sex, birth_date, body_weight] and a reason), \
`micronutrients` (state available: sex, age and NNR 2023 age_band, trailing_window, and per vitamin and mineral plus \
epa_dha: amount with its food and supplements shares, recommended_intake, adequate_intake (true for an NNR adequate \
intake, less certain), percent_of_recommended, and upper_limits, each with applies_to (total, supplements, \
preformed_vitamin_a, supplemental_folate, nicotinic_acid or nicotinamide), limit, the day's amount, seven_day_average \
over logged days and status none, caution (the day is above it) or cut_down (the 7-day average over at least 3 \
logged days with a known amount is above it); \
upper_limit_status is null for a nutrient without an Upper Limit; state unavailable: missing [sex, birth_date] and a \
reason, also below 18 years), `nutrients` \
with the total of every per-100 g database parameter in database order (key, Danish and English name, group, unit), \
`compounds` (named compounds without a parameter, such as creatine, with the amounts taken), `supplement_doses` (each \
Supplement's doses that day added together: supplement_id, name, dose_unit, doses), `nova` with nova_1_share (kcal from \
NOVA Class 1 Portions divided by kcal from classified Portions, 0-1; null when nothing classified has energy; Supplements \
are left out), nova_1_kcal, classified_kcal and unclassified_portions (left out of the share), and the day's meal_logs in \
time order. Each total is {value, unit, unknown_portions, food, supplements}: food adds the Portions that have a value, \
unknown_portions counts those that do not, supplements adds the Supplement Doses whose label lists it, value adds both \
(so energy and protein a label states count toward eaten), and each is null when nothing has one, including a day \
without Meal Logs. Nutrients a Supplement names by form also give supplement_forms [{form, value}], the supplement share \
of folic_acid, retinol, nicotinamide and so on. While the Food database is unavailable the result is a 503 error with code food_database_unavailable. Credit \
`source.credit` wherever values are shown.";

#[tool_router(router = meal_logs_router, vis = "pub(crate)")]
impl OpenHomeTools {
    #[tool(
        description = LOG_MEAL,
        annotations(
            title = "Log a meal",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn log_meal(&self, Parameters(request): Parameters<LogMealRequest>) -> CallToolResult {
        self.api
            .post(&["api", "nutrition", "meal-logs"], json!(request))
            .await
    }

    #[tool(
        description = MEAL_LOGS,
        annotations(title = "Meal Logs", read_only_hint = true, open_world_hint = false)
    )]
    async fn meal_logs(
        &self,
        Parameters(MealLogsRequest { day, from, to }): Parameters<MealLogsRequest>,
    ) -> CallToolResult {
        let query: Vec<(String, String)> = [("day", day), ("from", from), ("to", to)]
            .into_iter()
            .filter_map(|(name, value)| value.map(|value| (name.to_string(), value)))
            .collect();
        self.api
            .request(
                reqwest::Method::GET,
                &["api", "nutrition", "meal-logs"],
                None,
                &query,
            )
            .await
    }

    #[tool(
        description = UPDATE_MEAL_LOG,
        annotations(
            title = "Update a Meal Log",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn update_meal_log(
        &self,
        Parameters(request): Parameters<UpdateMealLogRequest>,
    ) -> CallToolResult {
        self.api
            .request(
                reqwest::Method::PATCH,
                &["api", "nutrition", "meal-logs", &request.meal_log_id],
                Some(json!(request)),
                &[],
            )
            .await
    }

    #[tool(
        description = DELETE_MEAL_LOG,
        annotations(
            title = "Delete a Meal Log",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn delete_meal_log(
        &self,
        Parameters(MealLogRequest { meal_log_id }): Parameters<MealLogRequest>,
    ) -> CallToolResult {
        self.api
            .request(
                reqwest::Method::DELETE,
                &["api", "nutrition", "meal-logs", &meal_log_id],
                None,
                &[],
            )
            .await
    }

    #[tool(
        description = NUTRITION_DAY,
        annotations(title = "Nutrition Day", read_only_hint = true, open_world_hint = false)
    )]
    async fn nutrition_day(
        &self,
        Parameters(NutritionDayRequest { day }): Parameters<NutritionDayRequest>,
    ) -> CallToolResult {
        self.api.get(&["api", "nutrition", "days", &day]).await
    }
}
