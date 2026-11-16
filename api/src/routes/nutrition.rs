//! Nutrition endpoints under `/api/nutrition`.

use crate::AppState;
use crate::services::nutrition::food_database::FoodDatabase;
use crate::services::nutrition::meal_photos::{MealPhotos, PHOTO_BODY_LIMIT};
use crate::services::nutrition::open_food_facts::OpenFoodFacts;
use axum::extract::DefaultBodyLimit;
use axum::handler::Handler;
use axum::routing::{get, put};
use axum::{Extension, Router};

mod catalog;
mod errors;
mod meal_logs;
mod meals;
mod overview;
mod supplements;
use catalog::{classify_food, food, link_product, lookup_product, search_foods};
use meal_logs::{
    delete_meal_log, list_meal_logs, log_meal, photo, thumbnail, update_meal_log, upload_photo,
};
use meals::{delete_meal, list_meals, save_meal, update_meal};
use overview::{nutrition_day, nutrition_goal, nutrition_summary, set_nutrition_goal};
use supplements::{add_supplement, delete_supplement, list_supplements, update_supplement};

pub fn router(foods: FoodDatabase, photos: MealPhotos, labels: OpenFoodFacts) -> Router<AppState> {
    // Only the routes that carry a photo raise Axum's default body limit.
    let photo_limit = DefaultBodyLimit::max(PHOTO_BODY_LIMIT);
    // Foods come from food.db with their NOVA Classes from app.db; Meals, Supplements, Meal Logs
    // and Products live in app.db and take values from food.db and Open Food Facts; Meal Logs keep photos in
    // the data directory.
    Router::new()
        .route("/api/nutrition/foods", get(search_foods))
        .route("/api/nutrition/foods/{food_id}", get(food))
        .route(
            "/api/nutrition/foods/{food_id}/nova-class",
            put(classify_food),
        )
        .route("/api/nutrition/products", axum::routing::post(link_product))
        .route("/api/nutrition/products/{barcode}", get(lookup_product))
        .route("/api/nutrition/meals", get(list_meals).post(save_meal))
        .route(
            "/api/nutrition/meals/{id}",
            axum::routing::patch(update_meal).delete(delete_meal),
        )
        .route(
            "/api/nutrition/supplements",
            get(list_supplements).post(add_supplement),
        )
        .route(
            "/api/nutrition/supplements/{id}",
            axum::routing::patch(update_supplement).delete(delete_supplement),
        )
        .route(
            "/api/nutrition/meal-logs",
            get(list_meal_logs).post(log_meal.layer(photo_limit)),
        )
        .route(
            "/api/nutrition/meal-logs/{id}",
            axum::routing::patch(update_meal_log.layer(photo_limit)).delete(delete_meal_log),
        )
        .route(
            "/api/nutrition/meal-logs/{id}/photo",
            put(upload_photo.layer(photo_limit)),
        )
        .route("/api/nutrition/meal-photos/{photo_id}", get(photo))
        .route(
            "/api/nutrition/meal-photos/{photo_id}/thumbnail",
            get(thumbnail),
        )
        .route("/api/nutrition/days/{day}", get(nutrition_day))
        .route("/api/nutrition/summary", get(nutrition_summary))
        .route(
            "/api/nutrition/goal",
            get(nutrition_goal).put(set_nutrition_goal),
        )
        .layer(Extension(foods))
        .layer(Extension(photos))
        .layer(Extension(labels))
}
