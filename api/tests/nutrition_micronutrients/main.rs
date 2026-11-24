#[path = "../food_workbook/builder.rs"]
mod builder;
#[path = "../common/memory_noop_db.rs"]
mod memory_noop_db;
#[path = "../common/send_request_with_method.rs"]
mod send_request_with_method;
#[path = "../nutrition_support/micronutrients.rs"]
mod support;
#[path = "../common/test_app_with_db.rs"]
mod test_app_with_db;
#[path = "../common/test_app_with_pools.rs"]
mod test_app_with_pools;

mod micronutrients;
mod summary;
mod upper_limits;
