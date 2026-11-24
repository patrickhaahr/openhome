#[path = "../food_workbook/builder.rs"]
mod builder;
#[path = "../food_workbook/dataset_v1.rs"]
mod dataset_v1;
#[path = "../common/memory_noop_db.rs"]
mod memory_noop_db;
#[path = "../common/send_request_with_method.rs"]
mod send_request_with_method;
#[path = "../nutrition_support/summary.rs"]
mod support;
#[path = "../common/test_app_with_db.rs"]
mod test_app_with_db;
#[path = "../common/test_app_with_pools.rs"]
mod test_app_with_pools;

mod summary;
mod targets;
