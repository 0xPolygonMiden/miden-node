mod blocks;
pub mod config;
pub mod db;
pub mod errors;
pub mod genesis;
mod nullifier_tree;
pub mod server;
pub mod state;
pub mod types;

// CONSTANTS
// =================================================================================================
pub const COMPONENT: &str = "miden-store";

/// Number of sql statements that each connection will cache.
const SQL_STATEMENT_CACHE_CAPACITY: usize = 32;
