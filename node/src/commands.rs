use std::{
    fs,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::anyhow;
use miden_crypto::{dsa::rpo_falcon512::KeyPair, utils::Serializable, Felt};
use miden_lib::{faucets::create_basic_fungible_faucet, wallets::create_basic_wallet, AuthScheme};
use miden_node_block_producer::server as block_producer_server;
use miden_node_rpc::server as rpc_server;
use miden_node_store::{db::Db, genesis::GenesisState, server as store_server};
use miden_node_utils::Config;
use miden_objects::assets::TokenSymbol;
use tokio::task::JoinSet;

use crate::{
    config::{NodeTopLevelConfig, CONFIG_FILENAME},
    DisplayPathBuf,
};

// START
// ===================================================================================================

pub async fn start() -> anyhow::Result<()> {
    let config: NodeTopLevelConfig =
        NodeTopLevelConfig::load_config(Some(Path::new(CONFIG_FILENAME))).extract()?;

    let mut join_set = JoinSet::new();
    let db = Db::setup(config.store.clone()).await?;
    join_set.spawn(store_server::api::serve(config.store, db));

    // wait for store before starting block producer
    tokio::time::sleep(Duration::from_secs(1)).await;
    join_set.spawn(block_producer_server::api::serve(config.block_producer));

    // wait for blockproducer before starting rpc
    tokio::time::sleep(Duration::from_secs(1)).await;
    join_set.spawn(rpc_server::api::serve(config.rpc));

    // block on all tasks
    while let Some(_res) = join_set.join_next().await {
        // do nothing
    }

    Ok(())
}

// MAKE GENESIS
// ===================================================================================================

/// Token symbol of the faucet present at genesis
const FUNGIBLE_FAUCET_TOKEN_SYMBOL: &str = "POL";

/// Decimals for the token of the faucet present at genesis
const FUNGIBLE_FAUCET_TOKEN_DECIMALS: u8 = 9;

/// Max supply for the token of the faucet present at genesis
const FUNGIBLE_FAUCET_TOKEN_MAX_SUPPLY: u64 = 1_000_000_000;

/// Seed for the Falcon512 keypair (faucet account)
const SEED_FAUCET_KEYPAIR: [u8; 40] = [2_u8; 40];

/// Seed for the Falcon512 keypair (wallet account)
const SEED_WALLET_KEYPAIR: [u8; 40] = [3_u8; 40];

/// Seed for the fungible faucet account
const SEED_FAUCET: [u8; 32] = [0_u8; 32];

/// Seed for the basic wallet account
const SEED_WALLET: [u8; 32] = [1_u8; 32];

/// Faucet account keys (public/private) file path
const FAUCET_KEYPAIR_FILE_PATH: &str = "faucet.fsk";

/// Wallet account keys (public/private) file path
const WALLET_KEYPAIR_FILE_PATH: &str = "wallet.fsk";

pub async fn make_genesis(
    output_path: &DisplayPathBuf,
    force: &bool,
) -> anyhow::Result<()> {
    let output_file_path = Path::new(&output_path.0);

    if !force {
        match output_file_path.try_exists() {
            Ok(file_exists) => {
                if file_exists {
                    return Err(anyhow!("Failed to generate new genesis file \"{output_path}\" because it already exists. Use the --force flag to overwrite."));
                }
            },
            Err(err) => {
                return Err(anyhow!(
                    "Failed to generate new genesis file \"{output_path}\". Error: {err}",
                ));
            },
        }
    }

    let faucet_key_pair = KeyPair::from_seed(&SEED_FAUCET_KEYPAIR).unwrap();
    let wallet_key_pair = KeyPair::from_seed(&SEED_WALLET_KEYPAIR).unwrap();

    let genesis_state = {
        let accounts = {
            let mut accounts = Vec::new();

            // fungible asset faucet
            {
                println!("Generating faucet account... ");
                let (account, _) = create_basic_fungible_faucet(
                    SEED_FAUCET,
                    TokenSymbol::new(FUNGIBLE_FAUCET_TOKEN_SYMBOL).unwrap(),
                    FUNGIBLE_FAUCET_TOKEN_DECIMALS,
                    Felt::from(FUNGIBLE_FAUCET_TOKEN_MAX_SUPPLY),
                    AuthScheme::RpoFalcon512 {
                        pub_key: faucet_key_pair.public_key(),
                    },
                )
                .unwrap();

                println!("Done");

                accounts.push(account);
            }

            // basic wallet account
            {
                println!("Generating basic wallet account... ");
                let (account, _) = create_basic_wallet(
                    SEED_WALLET,
                    AuthScheme::RpoFalcon512 {
                        pub_key: wallet_key_pair.public_key(),
                    },
                    miden_objects::accounts::AccountType::RegularAccountUpdatableCode,
                )
                .unwrap();

                println!("Done");

                accounts.push(account);
            }

            accounts
        };

        let version = 0;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("we are after 1970")
            .as_millis() as u64;

        GenesisState::new(accounts, version, timestamp)
    };

    // Write genesis state as binary format
    {
        let genesis_state_bin = genesis_state.to_bytes();

        fs::write(output_file_path, genesis_state_bin).unwrap_or_else(|_| {
            panic!("Failed to write genesis state to output file {}", output_file_path.display())
        });
    }

    // Write keypairs to disk
    fs::write(FAUCET_KEYPAIR_FILE_PATH, faucet_key_pair.to_bytes()).unwrap();
    fs::write(WALLET_KEYPAIR_FILE_PATH, wallet_key_pair.to_bytes()).unwrap();

    Ok(())
}
