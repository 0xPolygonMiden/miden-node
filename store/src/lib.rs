pub mod config;
pub mod db;
pub mod errors;
pub mod genesis;
pub mod server;
pub mod state;
pub mod types;

mod migrations;

#[macro_export]
macro_rules! target {
    () => {
        "miden-store"
    };
}
