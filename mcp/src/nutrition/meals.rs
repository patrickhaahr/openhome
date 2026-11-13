use rmcp::{handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router};
use serde_json::json;

use super::{MealRequest, SaveMealRequest, UpdateMealRequest};
use crate::OpenHomeTools;

const MEALS: &str = "List every saved Meal by name: id, name, portions in order, each with food_id (null for \
a Product), barcode (null for a Food), grams, the Food's Danish and English name (for a Product, the Food it is linked to) \
and product (barcode, name, brand) or null, and supplement_doses in order (supplement_id, name, dose_unit, doses). A Meal \
holds references, not nutrient values: values are copied when it is \
logged with log_meal and meal. Works while the Food database is unavailable.";

const SAVE_MEAL: &str = "Save a named Meal only when the user names it: up to 50 portions in order, each a food_id \
from search_foods or the barcode of a Product linked with link_product, listed once with edible grams, and up to 50 \
supplement_doses (supplement_id and doses, each Supplement once); at least one of either. The name (1-100 \
characters) is unique in any case. Returns meal (id, name, portions as meals lists them) and replayed. Generate a unique \
request_id per intended Meal and reuse identical arguments and ID after an uncertain response: an accepted retry returns \
replayed=true, changed arguments are a 409 error and a deleted Meal is not recreated. A name another Meal has is a 409 \
error; an unknown food_id, unlinked barcode or unknown supplement_id is a 404 error; while the Food database is \
unavailable, a Meal with portions is a 503 error with code food_database_unavailable.";

const UPDATE_MEAL: &str = "Rename a Meal or replace its items by meal_id. Supplied fields change together; omitted \
fields are preserved. name must stay unique in any case (another Meal's name is a 409 error); portions replaces every item \
(food_id or a linked Product's barcode with edible grams, each once) and supplement_doses every dose (supplement_id and \
doses); at least one item or dose must remain. Returns the updated Meal. Past Meal Logs made from \
the Meal keep their Portions and meal_name. A missing Meal, food_id, unlinked barcode or supplement_id is a 404 error; replacing items \
while the Food database is unavailable is a 503 error. After an uncertain response, list meals before retrying.";

const DELETE_MEAL: &str = "Permanently delete one Meal by meal_id. Meal Logs made from it are unchanged. Returns \
deleted=true only after the API confirms removal; a missing Meal is a 404 error. An old save_meal retry with its \
request_id cannot recreate it. After an uncertain response, list meals before retrying.";

#[tool_router(router = meals_router, vis = "pub(crate)")]
impl OpenHomeTools {
    #[tool(
        description = MEALS,
        annotations(title = "Meals", read_only_hint = true, open_world_hint = false)
    )]
    async fn meals(&self) -> CallToolResult {
        self.api.get(&["api", "nutrition", "meals"]).await
    }

    #[tool(
        description = SAVE_MEAL,
        annotations(
            title = "Save a Meal",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn save_meal(&self, Parameters(request): Parameters<SaveMealRequest>) -> CallToolResult {
        self.api
            .post(&["api", "nutrition", "meals"], json!(request))
            .await
    }

    #[tool(
        description = UPDATE_MEAL,
        annotations(
            title = "Update a Meal",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn update_meal(
        &self,
        Parameters(request): Parameters<UpdateMealRequest>,
    ) -> CallToolResult {
        self.api
            .request(
                reqwest::Method::PATCH,
                &["api", "nutrition", "meals", &request.meal_id],
                Some(json!(request)),
                &[],
            )
            .await
    }

    #[tool(
        description = DELETE_MEAL,
        annotations(
            title = "Delete a Meal",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn delete_meal(
        &self,
        Parameters(MealRequest { meal_id }): Parameters<MealRequest>,
    ) -> CallToolResult {
        self.api
            .request(
                reqwest::Method::DELETE,
                &["api", "nutrition", "meals", &meal_id],
                None,
                &[],
            )
            .await
    }
}
