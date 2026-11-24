#[path = "../common/memory_noop_db.rs"]
mod memory_noop_db;
#[path = "../common/test_app_with_db.rs"]
mod test_app_with_db;
#[path = "../common/test_app_with_pools.rs"]
mod test_app_with_pools;

mod support;

mod append_batches;
mod append_limits;
mod append_validation;
mod handshake;
mod replace_apply;
mod replace_generations;
mod replace_staging;
mod replace_validation;
