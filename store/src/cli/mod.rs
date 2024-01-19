use std::{path::PathBuf, str::FromStr};

use anyhow::anyhow;
use clap::{Args, Parser, Subcommand};
use hex::FromHex;
use miden_crypto::{Felt, FieldElement, StarkField};
use miden_node_proto::digest::Digest;
use miden_node_store::config;

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long, value_name = "FILE", default_value = config::CONFIG_FILENAME)]
    pub config: PathBuf,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Subcommand)]
pub enum Command {
    /// Starts the Store gRPC service.
    Serve,

    #[command(subcommand)]
    /// Queries the Store via gRPC.
    Query(Query),

    #[command(subcommand)]
    /// Administer the Store via gRPC.
    Admin(Admin),
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Subcommand)]
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

#[derive(Args, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct GetBlockHeaderByNumberArgs {
    /// Optional block height, if unspecified return latest.
    pub block_num: Option<u32>,
}

#[derive(Args, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct CheckNullifiersArgs {
    /// List of nullifiers to query.
    #[arg(value_parser=parse_nullifier)]
    pub nullifiers: Vec<Digest>,
}

#[derive(Args, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct SyncStateArgs {
    /// List of accounts to include in the result
    #[arg(long="account", value_parser=parse_account_id)]
    pub account_ids: Vec<u64>,

    /// List of prefixes to filter notes.
    #[arg(long = "note")]
    pub note_tags: Vec<u32>,

    /// List of prefixes to filter nullifiers.
    #[arg(long = "nullifier")]
    pub nullifiers: Vec<u32>,

    /// Start block height.
    pub block_num: u32,
}

#[derive(Args, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct GetBlockInputsArgs {
    /// List of account ids to query.
    #[arg(long="account", value_parser=parse_account_id)]
    pub account_ids: Vec<u64>,

    /// List of nullifiers to query.
    #[arg(value_parser=parse_nullifier)]
    pub nullifiers: Vec<Digest>,
}

#[derive(Args, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct GetTransactionInputsArgs {
    /// Account id to query.
    #[arg(value_parser=parse_account_id)]
    pub account_id: u64,

    /// Nullifiers to query.
    #[arg(value_parser=parse_nullifier)]
    pub nullifiers: Vec<Digest>,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Subcommand)]
pub enum Admin {
    /// Starts a server clean sthudown.
    Shutdown,
}

/// Parses an `u64` used to repesent an account id, returns an error if the u64 doesn't fit in the
/// field's modulus.
fn parse_account_id(value: &str) -> anyhow::Result<u64> {
    let number = <Felt as FieldElement>::PositiveInteger::from_str(value)?;
    if number >= Felt::MODULUS {
        return Err(anyhow!("Account id larger than field modulus"));
    }
    Ok(number)
}

/// Parses a hex-encoded digest from the slice.
fn parse_nullifier(value: &str) -> Result<Digest, String> {
    Digest::from_hex(value.as_bytes()).map_err(|e| e.to_string())
}
