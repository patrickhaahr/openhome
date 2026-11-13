use rmcp::{handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router};
use serde_json::json;

use super::{
    ClassifyFoodRequest, FoodRequest, LinkProductRequest, LookupProductRequest, SearchFoodsRequest,
};
use crate::OpenHomeTools;

const SEARCH_FOODS: &str = "Search Foods in the Danish Food Composition Database by words of their \
Danish or English name (skyr, kyllingebryst, chicken breast); Danish compounds also match the database's comma-separated \
names. Returns up to `limit` Foods (1-50, default 20), best match first, each with food_id, name_da, name_en, food_group, \
`per_100g` key values per 100 g edible part (energy_kcal, energy_kj, protein, fat, available_carbohydrate, sugars, \
dietary_fibre, alcohol), `density` in g/ml and `waste` as the inedible percentage of the Food as bought, each as {value, \
unit}, and the user's `nova_class` (1-4, null while unclassified); a null value is unknown, never zero. An empty `foods` list means no match: try other words, the other language, or \
the dish's separate Foods. While the database is being imported, or after an import failed, the result is a 503 error \
with code food_database_unavailable and state importing or failed. Credit `source.credit` wherever values are shown.";

const FOOD: &str = "One Food by the food_id search_foods returned, with every database parameter in \
`parameters`, in database order: key, parameter_id, Danish and English name, group, unit, per_100g and value. A per_100g \
value is an amount per 100 g edible part; waste (%), density (g/ml), conversion factors and pH describe the Food itself. A \
null value is unknown, never zero. `density`, `waste` and the user's `nova_class` (1-4, null while unclassified) are at \
the top level. An unknown food_id is a 404 error; \
while the database is unavailable the result is a 503 error with code food_database_unavailable. Credit `source.credit` \
wherever values are shown.";

const CLASSIFY_FOOD: &str = "Set or change a Food's NOVA Class by food_id: 1 unprocessed or minimally processed, 2 \
processed culinary ingredient, 3 processed, 4 ultra-processed. Classify a Food once, the first time it is logged while \
search_foods shows nova_class null; OpenHome keeps the class with the user's data, so a Food database update never loses \
it. The Food's past Portions that were still unclassified take the class; a changed class applies to Portions logged \
afterwards. Products use Open Food Facts' class instead. Returns food_id, name_da, name_en, nova_class and \
classified_portions (past Portions that took the class). Setting the same class again changes nothing. A class outside 1-4 \
is a 400 error, an unknown food_id a 404 error, and while the Food database is unavailable the result is a 503 error with \
code food_database_unavailable.";

const LOOKUP_PRODUCT: &str = "Look up a packaged food by the 8, 12, 13 or 14 digits under its bars. A \
barcode already linked returns linked=true and the stored Product (barcode, name, brand, nova_class, label_per_100g as \
{key, value, unit} in Food database units, and its linked food) without calling Open Food Facts. Otherwise it returns \
linked=false: a preview read from Open Food Facts with food null and up to 5 suggested_foods (as search_foods lists them) \
to link it to; nothing is stored until link_product. A malformed barcode or wrong check digit is a 400 error; a barcode \
Open Food Facts does not know is a 404 error with code product_not_found (log the closest Food instead); Open Food Facts \
not answering is a 503 error with code open_food_facts_unavailable; a preview while the Food database is unavailable is a \
503 error with code food_database_unavailable. Credit `source.credit` (Open Food Facts, ODbL) wherever label values are \
shown.";

const LINK_PRODUCT: &str = "Link a barcode to its closest Food once, storing the Product with its label values per \
100 g, NOVA Class and food_id; later lookups and Portions use it without calling Open Food Facts. Linking a stored Product \
again re-links it to another Food for future Portions only; past Meal Logs keep their values. Returns product (as \
lookup_product shows it) and replayed. Generate a unique request_id per intended link and reuse identical arguments and \
ID after an uncertain response: an accepted retry returns replayed=true and the same request_id with another barcode or \
food_id is a 409 error. An unknown food_id is a 404 error; a barcode Open Food Facts does not know is a 404 error with \
code product_not_found; Open Food Facts not answering is a 503 error with code open_food_facts_unavailable.";

#[tool_router(router = foods_router, vis = "pub(crate)")]
impl OpenHomeTools {
    #[tool(
        description = SEARCH_FOODS,
        annotations(title = "Search Foods", read_only_hint = true, open_world_hint = false)
    )]
    async fn search_foods(
        &self,
        Parameters(SearchFoodsRequest { query, limit }): Parameters<SearchFoodsRequest>,
    ) -> CallToolResult {
        let mut parameters = vec![("query".to_string(), query)];
        if let Some(limit) = limit {
            parameters.push(("limit".into(), limit.to_string()));
        }
        self.api
            .request(
                reqwest::Method::GET,
                &["api", "nutrition", "foods"],
                None,
                &parameters,
            )
            .await
    }

    #[tool(
        description = FOOD,
        annotations(title = "Food", read_only_hint = true, open_world_hint = false)
    )]
    async fn food(
        &self,
        Parameters(FoodRequest { food_id }): Parameters<FoodRequest>,
    ) -> CallToolResult {
        self.api
            .get(&["api", "nutrition", "foods", &food_id.to_string()])
            .await
    }

    #[tool(
        description = CLASSIFY_FOOD,
        annotations(
            title = "Classify a Food",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn classify_food(
        &self,
        Parameters(request): Parameters<ClassifyFoodRequest>,
    ) -> CallToolResult {
        self.api
            .request(
                reqwest::Method::PUT,
                &[
                    "api",
                    "nutrition",
                    "foods",
                    &request.food_id.to_string(),
                    "nova-class",
                ],
                Some(json!(request)),
                &[],
            )
            .await
    }

    #[tool(
        description = LOOKUP_PRODUCT,
        annotations(title = "Look up a Product", read_only_hint = true, open_world_hint = true)
    )]
    async fn lookup_product(
        &self,
        Parameters(LookupProductRequest { barcode }): Parameters<LookupProductRequest>,
    ) -> CallToolResult {
        self.api
            .get(&["api", "nutrition", "products", &barcode])
            .await
    }

    #[tool(
        description = LINK_PRODUCT,
        annotations(
            title = "Link a Product",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = true
        )
    )]
    async fn link_product(
        &self,
        Parameters(request): Parameters<LinkProductRequest>,
    ) -> CallToolResult {
        self.api
            .post(&["api", "nutrition", "products"], json!(request))
            .await
    }
}
