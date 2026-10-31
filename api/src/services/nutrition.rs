//! Nutrition (`CONTEXT.md`, Nutrition): the Food database, the parameter vocabulary shared by
//! everything that carries nutrient values, Products read from Open Food Facts, NOVA Classes, named
//! Meals, Supplements, Meal Logs with their photos, the Nutrition Goal and the targets derived
//! from it, the micronutrient evaluation against NNR 2023 and EFSA references, and the Nutrition
//! Day built from them.

pub mod day;
pub mod food_database;
pub mod goals;
pub mod intake;
pub mod meal_logs;
pub mod meal_photos;
pub mod meals;
pub mod micronutrients;
pub mod nova;
pub mod open_food_facts;
pub mod parameters;
pub mod products;
pub mod supplements;
pub mod targets;
