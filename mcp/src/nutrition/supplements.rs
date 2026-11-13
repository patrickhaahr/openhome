use rmcp::{handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router};
use serde_json::json;

use super::{AddSupplementRequest, SupplementRequest, UpdateSupplementRequest};
use crate::OpenHomeTools;

const SUPPLEMENTS: &str = "List every Supplement by name: id, name, dose_unit (tablet, capsule, scoop or ml), \
amounts per dose (key, form or null, value, unit in the Food database parameter's unit) and compounds per dose (name, \
value, unit). Works while the Food database is unavailable.";

const ADD_SUPPLEMENT: &str = "Register a Supplement from its label once, before logging its first dose: a name unique in \
any case, dose_unit (tablet, capsule, scoop or ml) and at least one per-dose amount or compound. amounts take Food database \
parameter keys (vitamin_d, vitamin_b12, zinc, magnesium, iodine, epa, dha, energy_kcal, protein, ...) with the label's unit \
(the parameter's unit, or g, mg or µg for masses; convert IU first); vitamin_a, folate, niacin_equivalent and niacin need \
form. Other compounds (creatine) go in compounds with g, mg or µg; a parameter named as a compound is refused. Energy in kcal \
or kJ is completed from the other. Never register protein powder: it is a Product logged in grams. Returns supplement (id, \
name, dose_unit, amounts, compounds) and replayed. Generate a unique request_id per intended Supplement and reuse identical \
arguments and ID after an uncertain response: an accepted retry returns replayed=true, changed arguments are a 409 error \
and a deleted Supplement is not recreated. A name another Supplement has is a 409 error; an unknown key, wrong unit or \
missing form is a 400 error.";

const UPDATE_SUPPLEMENT: &str = "Correct a Supplement by supplement_id: name, dose_unit, or a full replacement of its \
amounts or compounds. Supplied fields change together; omitted fields are preserved; none may be null, and at least one \
amount or compound must remain. Returns the updated Supplement. Past Meal Logs keep the doses they copied; Meals that take \
it log its new label from now on. A missing Supplement is a 404 error and another Supplement's name a 409 error. After an \
uncertain response, list supplements before retrying.";

const DELETE_SUPPLEMENT: &str = "Permanently delete one Supplement by supplement_id. Past Meal Logs keep the doses they \
copied. Returns deleted=true only after the API confirms removal; a missing Supplement is a 404 error, and a Supplement a \
Meal takes is a 409 error naming the Meals (update_meal them first). An old add_supplement retry with its request_id \
cannot recreate it. After an uncertain response, list supplements before retrying.";

#[tool_router(router = supplements_router, vis = "pub(crate)")]
impl OpenHomeTools {
    #[tool(
        description = SUPPLEMENTS,
        annotations(title = "Supplements", read_only_hint = true, open_world_hint = false)
    )]
    async fn supplements(&self) -> CallToolResult {
        self.api.get(&["api", "nutrition", "supplements"]).await
    }

    #[tool(
        description = ADD_SUPPLEMENT,
        annotations(
            title = "Add a Supplement",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn add_supplement(
        &self,
        Parameters(request): Parameters<AddSupplementRequest>,
    ) -> CallToolResult {
        self.api
            .post(&["api", "nutrition", "supplements"], json!(request))
            .await
    }

    #[tool(
        description = UPDATE_SUPPLEMENT,
        annotations(
            title = "Update a Supplement",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn update_supplement(
        &self,
        Parameters(request): Parameters<UpdateSupplementRequest>,
    ) -> CallToolResult {
        self.api
            .request(
                reqwest::Method::PATCH,
                &["api", "nutrition", "supplements", &request.supplement_id],
                Some(json!(request)),
                &[],
            )
            .await
    }

    #[tool(
        description = DELETE_SUPPLEMENT,
        annotations(
            title = "Delete a Supplement",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn delete_supplement(
        &self,
        Parameters(SupplementRequest { supplement_id }): Parameters<SupplementRequest>,
    ) -> CallToolResult {
        self.api
            .request(
                reqwest::Method::DELETE,
                &["api", "nutrition", "supplements", &supplement_id],
                None,
                &[],
            )
            .await
    }
}
