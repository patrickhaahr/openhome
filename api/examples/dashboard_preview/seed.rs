//! Seeds the main database through the production HTTP routes: profile, body weight, logged
//! workouts, Household Users, Wishlists and the Watchlist. Wish images point at the preview's own
//! artwork routes (`media.rs`); Watchlist posters are static TMDB paths from the catalogue.

use std::collections::HashMap;

use chrono::{Days, NaiveDate, Utc};
use chrono_tz::Europe::Copenhagen;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{Value, json};

pub fn today() -> NaiveDate {
    Utc::now().with_timezone(&Copenhagen).date_naive()
}

fn days_ago(days: u64) -> String {
    today()
        .checked_sub_days(Days::new(days))
        .expect("seed dates are recent")
        .to_string()
}

struct Api {
    client: Client,
    base: String,
    key: String,
}

impl Api {
    async fn send(
        &self,
        method: reqwest::Method,
        path: &str,
        body: &Value,
    ) -> anyhow::Result<Value> {
        let mut request = self
            .client
            .request(method.clone(), format!("{}{path}", self.base))
            .bearer_auth(&self.key);
        if !body.is_null() {
            request = request.json(body);
        }
        let response = request.send().await?;
        let status = response.status();
        let text = response.text().await?;
        anyhow::ensure!(status.is_success(), "{method} {path} -> {status}: {text}");
        Ok(serde_json::from_str(&text).unwrap_or(Value::Null))
    }

    async fn get(&self, path: &str) -> anyhow::Result<Value> {
        self.send(reqwest::Method::GET, path, &Value::Null).await
    }

    async fn post(&self, path: &str, body: Value) -> anyhow::Result<Value> {
        self.send(reqwest::Method::POST, path, &body).await
    }

    async fn patch(&self, path: &str, body: Value) -> anyhow::Result<Value> {
        self.send(reqwest::Method::PATCH, path, &body).await
    }
}

fn id_of(value: &Value) -> anyhow::Result<&str> {
    value["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("response has no id: {value}"))
}

pub async fn household(base: &str, key: &str, db: &sqlx::SqlitePool) -> anyhow::Result<()> {
    let api = Api {
        client: Client::new(),
        base: base.to_string(),
        key: key.to_string(),
    };
    body(&api).await?;
    workouts(&api).await?;
    household_lists(&api, db).await
}

#[derive(Deserialize)]
struct Body {
    height_cm: f64,
    sex: String,
    weights_kg_oldest_first: Vec<f64>,
}

async fn body(api: &Api) -> anyhow::Result<()> {
    let seed: Body = serde_json::from_str(include_str!("seed/body.json"))?;
    let training: Value = serde_json::from_str(include_str!("seed/training.json"))?;
    api.patch(
        "/api/profile",
        json!({ "height_cm": seed.height_cm, "sex": seed.sex, "hr_max_bpm": training["hr_max_bpm"] }),
    )
    .await?;
    let count = seed.weights_kg_oldest_first.len() as u64;
    for (index, weight) in seed.weights_kg_oldest_first.iter().enumerate() {
        api.post(
            "/api/body_weight",
            json!({ "date": days_ago(count - 1 - index as u64), "weight_kg": weight }),
        )
        .await?;
    }
    Ok(())
}

#[derive(Deserialize)]
struct Workouts {
    exercises: Vec<Value>,
    sessions: Vec<Session>,
}

#[derive(Deserialize)]
struct Session {
    days_ago: u64,
    name: String,
    notes: Option<String>,
    exercises: Vec<LoggedExercise>,
}

#[derive(Deserialize)]
struct LoggedExercise {
    exercise: String,
    sets: Vec<Value>,
}

async fn workouts(api: &Api) -> anyhow::Result<()> {
    let seed: Workouts = serde_json::from_str(include_str!("seed/workouts.json"))?;
    // Migrations already seed a catalogue; only exercises it lacks are created.
    let mut ids: HashMap<String, i64> = api
        .get("/api/exercises")
        .await?
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|exercise| {
            Some((
                exercise["name"].as_str()?.to_string(),
                exercise["id"].as_i64()?,
            ))
        })
        .collect();
    for exercise in &seed.exercises {
        let name = exercise["name"].as_str().unwrap_or_default().to_string();
        if let std::collections::hash_map::Entry::Vacant(slot) = ids.entry(name) {
            let created = api.post("/api/exercises", exercise.clone()).await?;
            slot.insert(created["id"].as_i64().unwrap_or_default());
        }
    }
    for session in &seed.sessions {
        let exercises = session
            .exercises
            .iter()
            .enumerate()
            .map(|(order, logged)| {
                let id = ids
                    .get(&logged.exercise)
                    .ok_or_else(|| anyhow::anyhow!("unknown exercise {}", logged.exercise))?;
                let sets: Vec<Value> = logged
                    .sets
                    .iter()
                    .enumerate()
                    .map(|(number, set)| {
                        let mut set = set.clone();
                        set["set_number"] = json!(number + 1);
                        set
                    })
                    .collect();
                Ok(json!({ "exercise_id": id, "order_index": order, "sets": sets }))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        api.post(
            "/api/workouts",
            json!({
                "date": days_ago(session.days_ago),
                "name": session.name,
                "notes": session.notes,
                "exercises": exercises,
            }),
        )
        .await?;
    }
    Ok(())
}

#[derive(Deserialize)]
struct Household {
    users: Vec<User>,
    watchlist: Vec<Watch>,
}

#[derive(Deserialize)]
struct User {
    name: String,
    wishlists: Vec<Wishlist>,
}

#[derive(Deserialize)]
struct Wishlist {
    name: String,
    items: Vec<Value>,
}

#[derive(Deserialize)]
struct Watch {
    owner: String,
    kind: String,
    title: String,
    status: String,
    tmdb_id: Option<i64>,
    added_days_ago: u64,
    completed_days_ago: Option<u64>,
}

async fn household_lists(api: &Api, db: &sqlx::SqlitePool) -> anyhow::Result<()> {
    let seed: Household = serde_json::from_str(include_str!("seed/household.json"))?;
    let catalogue = crate::media::catalogue();
    let mut owners = HashMap::new();
    let mut request = 0;
    for user in &seed.users {
        let created = api
            .post("/api/household-users", json!({ "name": user.name }))
            .await?;
        let owner_id = id_of(&created)?.to_string();
        for list in &user.wishlists {
            let created = api
                .post(
                    "/api/wishlists",
                    json!({ "owner_id": owner_id, "name": list.name }),
                )
                .await?;
            let list_id = id_of(&created)?.to_string();
            for item in &list.items {
                request += 1;
                let mut item = item.clone();
                // `art` names a drawing served by the preview itself; it is not an API field.
                if let Some(art) = item
                    .as_object_mut()
                    .and_then(|fields| fields.remove("art"))
                    .and_then(|art| art.as_str().map(str::to_string))
                {
                    item["image_url"] = json!(format!("{}/preview-art/wish/{art}.svg", api.base));
                }
                item["owner_id"] = json!(owner_id);
                item["list_id"] = json!(list_id);
                item["request_id"] = json!(format!("preview-wish-{request}"));
                api.post("/api/wishlist/items", item).await?;
            }
        }
        owners.insert(user.name.clone(), owner_id);
    }
    for (index, watch) in seed.watchlist.iter().enumerate() {
        let owner_id = owners
            .get(&watch.owner)
            .ok_or_else(|| anyhow::anyhow!("unknown watchlist owner {}", watch.owner))?;
        let entry = json!({
            "owner_id": owner_id,
            "kind": watch.kind,
            "title": watch.title,
            "request_id": format!("preview-watch-{index}"),
        });
        let created = api.post("/api/watchlist/entries", entry).await?;
        let entry_id = id_of(&created["entry"])?.to_string();
        // Static catalogue metadata goes straight into the row: adding with a TMDB id would
        // make the API call TMDB for every seeded entry at startup.
        if let Some(title) = watch
            .tmdb_id
            .and_then(|id| catalogue.iter().find(|title| title.id == id))
        {
            sqlx::query("UPDATE watchlist_entries SET tmdb_id = ?, overview = ?, poster_path = ?, release_date = ? WHERE id = ?")
                .bind(title.id)
                .bind(&title.overview)
                .bind(&title.poster_path)
                .bind(&title.date)
                .bind(&entry_id)
                .execute(db)
                .await?;
        }
        let mut edit =
            json!({ "status": watch.status, "date_added": days_ago(watch.added_days_ago) });
        if let Some(completed) = watch.completed_days_ago {
            edit["completion_date"] = json!(days_ago(completed));
        }
        api.patch(
            &format!("/api/household-users/{owner_id}/watchlist/entries/{entry_id}"),
            edit,
        )
        .await?;
    }
    Ok(())
}
