use super::{
    FoodDatabase, FoodError, FoodSnapshot, NutrientParameter, ParameterGroup, SnapshotValue,
    Source, source,
};
use sqlx::{QueryBuilder, Row};
use std::collections::HashMap;

impl FoodDatabase {
    pub async fn names(
        &self,
        food_ids: &[i64],
    ) -> Result<HashMap<i64, (String, String)>, FoodError> {
        self.ready()?;
        let mut names = HashMap::new();
        if food_ids.is_empty() {
            return Ok(names);
        }
        let mut query =
            QueryBuilder::new("SELECT food_id, name_da, name_en FROM food WHERE food_id IN (");
        let mut separated = query.separated(", ");
        for id in food_ids {
            separated.push_bind(*id);
        }
        query.push(")");
        for row in query.build().fetch_all(&self.pool).await? {
            names.insert(row.get("food_id"), (row.get("name_da"), row.get("name_en")));
        }
        Ok(names)
    }

    pub async fn snapshots(
        &self,
        food_ids: &[i64],
    ) -> Result<HashMap<i64, FoodSnapshot>, FoodError> {
        let ready = self.ready()?;
        let mut ids = food_ids.to_vec();
        ids.sort_unstable();
        ids.dedup();
        let mut snapshots = HashMap::new();
        if ids.is_empty() {
            return Ok(snapshots);
        }
        let mut query =
            QueryBuilder::new("SELECT food_id, name_da, name_en FROM food WHERE food_id IN (");
        let mut separated = query.separated(", ");
        for id in &ids {
            separated.push_bind(*id);
        }
        query.push(")");
        for row in query.build().fetch_all(&self.pool).await? {
            let food_id: i64 = row.get("food_id");
            snapshots.insert(
                food_id,
                FoodSnapshot {
                    food_id,
                    name_da: row.get("name_da"),
                    name_en: row.get("name_en"),
                    dataset_version: ready.version.clone(),
                    per_100g: Vec::new(),
                },
            );
        }
        if let Some(missing) = ids.iter().find(|id| !snapshots.contains_key(id)) {
            return Err(FoodError::NotFound(*missing));
        }
        let mut query = QueryBuilder::new(
            "SELECT v.food_id, p.key, v.value, p.unit FROM food_value v \
             JOIN parameter p ON p.parameter_id = v.parameter_id \
             WHERE p.per_100g = 1 AND p.unit IS NOT NULL AND v.food_id IN (",
        );
        let mut separated = query.separated(", ");
        for id in &ids {
            separated.push_bind(*id);
        }
        query.push(") ORDER BY p.sort_key, p.parameter_id");
        for row in query.build().fetch_all(&self.pool).await? {
            if let Some(snapshot) = snapshots.get_mut(&row.get::<i64, _>("food_id")) {
                snapshot.per_100g.push(SnapshotValue {
                    key: row.get("key"),
                    value: row.get("value"),
                    unit: row.get("unit"),
                });
            }
        }
        Ok(snapshots)
    }

    pub async fn nutrient_parameters(&self) -> Result<(Vec<NutrientParameter>, Source), FoodError> {
        let ready = self.ready()?;
        let parameters = sqlx::query(
            "SELECT key, name_da, name_en, group_da, group_en, unit FROM parameter \
             WHERE per_100g = 1 ORDER BY sort_key, parameter_id",
        )
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(|row| NutrientParameter {
            key: row.get("key"),
            name_da: row.get("name_da"),
            name_en: row.get("name_en"),
            group: ParameterGroup {
                name_da: row.get("group_da"),
                name_en: row.get("group_en"),
            },
            unit: row.get("unit"),
        })
        .collect();
        Ok((parameters, source(&ready.version)))
    }
}
