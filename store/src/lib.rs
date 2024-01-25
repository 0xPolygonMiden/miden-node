pub mod config;
pub mod db;
pub mod errors;
pub mod genesis;
pub mod server;
pub mod state;
pub mod types;

#[macro_export]
macro_rules! target {
    () => {
        "miden-store"
    };
}

// CONSTANTS
// =================================================================================================
pub const COMPONENT: &str = target!();
