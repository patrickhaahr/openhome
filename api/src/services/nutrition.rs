//! Nutrition (`CONTEXT.md`, Nutrition): the Food database, the parameter vocabulary shared by
//! everything that carries nutrient values, Products read from Open Food Facts, NOVA Classes, named
//! Meals, Meal Logs with their photos and the Nutrition Day built from them.

pub mod day;
pub mod food_database;
pub mod meal_logs;
pub mod meal_photos;
pub mod meals;
pub mod nova;
pub mod open_food_facts;
pub mod parameters;
pub mod products;
