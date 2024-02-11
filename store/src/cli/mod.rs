use std::{path::PathBuf, str::FromStr};

use clap::{Args, Parser, Subcommand};
use miden_node_store::config;
use miden_objects::{accounts::AccountId, notes::Nullifier};

// CLI COMMANDS
// ================================================================================================

#[derive(Clone, Eq, PartialEq, Debug, Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long, value_name = "FILE", default_value = config::CONFIG_FILENAME)]
    pub config: PathBuf,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Clone, Eq, PartialEq, Debug, Subcommand)]
pub enum Command {
    /// Starts the Store gRPC service.
    Serve,

    #[command(subcommand)]
    /// Queries the Store via gRPC.
    Query(Query),
}

#[derive(Clone, Eq, PartialEq, Debug, Subcommand)]
pub enum Query {
    /// Query a block header.
    GetBlockHeaderByNumber(GetBlockHeaderByNumberArgs),

    /// Query the state of some nullifiers.
    CheckNullifiers(CheckNullifiersArgs),

    /// Query state update of a client.
    SyncState(SyncStateArgs),

    /// Query inputs to create a block.
    GetBlockInputs(GetBlockInputsArgs),

    /// Query inputs to create a transaction.
    GetTransactionInputs(GetTransactionInputsArgs),

    /// Query all known nullifiers.
    ListNullifiers,

    /// Query all known notes.
    ListNotes,

    /// Query all known accounts.
    ListAccounts,
}

// COMMAND ARGS
// ================================================================================================

#[derive(Args, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct GetBlockHeaderByNumberArgs {
    /// Optional block height, if unspecified return latest.
    pub block_num: Option<u32>,
}

#[derive(Args, Clone, Eq, PartialEq, Debug)]
pub struct CheckNullifiersArgs {
    /// List of nullifiers to query.
    #[arg(value_parser=parse_nullifier)]
    pub nullifiers: Vec<Nullifier>,
}

#[derive(Args, Clone, Eq, PartialEq, Debug)]
pub struct SyncStateArgs {
    /// List of accounts to include in the result
    #[arg(long="account", value_parser=parse_account_id)]
    pub account_ids: Vec<AccountId>,

    /// List of prefixes to filter notes.
    #[arg(long = "note")]
    pub note_tags: Vec<u32>,

    /// List of prefixes to filter nullifiers.
    #[arg(long = "nullifier")]
    pub nullifiers: Vec<u32>,

    /// Start block height.
    pub block_num: u32,
}

#[derive(Args, Clone, Eq, PartialEq, Debug)]
pub struct GetBlockInputsArgs {
    /// List of account ids to query.
    #[arg(long="account", value_parser=parse_account_id)]
    pub account_ids: Vec<AccountId>,

    /// List of nullifiers to query.
    #[arg(value_parser=parse_nullifier)]
    pub nullifiers: Vec<Nullifier>,
}

#[derive(Args, Clone, Eq, PartialEq, Debug)]
pub struct GetTransactionInputsArgs {
    /// Account id to query.
    #[arg(value_parser=parse_account_id)]
    pub account_id: AccountId,

    /// Nullifiers to query.
    #[arg(value_parser=parse_nullifier)]
    pub nullifiers: Vec<Nullifier>,
}

// HELPER FUNCTIONS
// ================================================================================================

/// Parses an `u64` used to represent an account id, returns an error if the u64 doesn't fit in
/// the field's modulus.
fn parse_account_id(value: &str) -> anyhow::Result<AccountId> {
    let number = u64::from_str(value)?;
    Ok(number.try_into()?)
}

/// Parses a hex-encoded digest from the slice.
fn parse_nullifier(value: &str) -> Result<Nullifier, String> {
    Nullifier::from_hex(value).map_err(|e| e.to_string())
}
