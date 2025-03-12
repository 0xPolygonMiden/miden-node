mod blocks;
pub mod db;
pub mod errors;
pub mod genesis;
mod nullifier_tree;
pub mod server;
pub mod state;

// CONSTANTS
// =================================================================================================
pub const COMPONENT: &str = "miden-store";
pub const GENESIS_STATE_FILENAME: &str = "genesis.dat";

/// Number of sql statements that each connection will cache.
const SQL_STATEMENT_CACHE_CAPACITY: usize = 32;
