use std::{
    any::Any,
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::anyhow;
use miden_crypto::{dsa::rpo_falcon512::KeyPair, utils::Serializable, Felt, Word};
use miden_lib::{
    accounts::{faucets::create_basic_fungible_faucet, wallets::create_basic_wallet},
    AuthScheme,
};
use miden_node_block_producer::server as block_producer_server;
use miden_node_rpc::server as rpc_server;
use miden_node_store::{
    db::Db,
    genesis::{AccountAndSeed, GenesisState},
    server as store_server,
};
use miden_node_utils::config::load_config;
use miden_objects::{accounts::Account, assets::TokenSymbol};
use tokio::task::JoinSet;

use crate::{
    config::NodeTopLevelConfig,
    genesis::{
        FAUCET_KEYPAIR_FILE_PATH, FUNGIBLE_FAUCET_TOKEN_DECIMALS, FUNGIBLE_FAUCET_TOKEN_MAX_SUPPLY,
        FUNGIBLE_FAUCET_TOKEN_SYMBOL, SEED_FAUCET, SEED_FAUCET_KEYPAIR, SEED_WALLET,
        SEED_WALLET_KEYPAIR, WALLET_KEYPAIR_FILE_PATH,
    },
};

// START
// ===================================================================================================

pub async fn start(config_filepath: &Path) -> anyhow::Result<()> {
    let config: NodeTopLevelConfig = load_config(config_filepath).extract().map_err(|err| {
        anyhow!("failed to load config file `{}`: {err}", config_filepath.display())
    })?;

    let mut join_set = JoinSet::new();
    let db = Db::setup(config.store.clone()).await?;
    join_set.spawn(store_server::serve(config.store, db));

    // wait for store before starting block producer
    tokio::time::sleep(Duration::from_secs(1)).await;
    join_set.spawn(block_producer_server::serve(config.block_producer));

    // wait for block producer before starting rpc
    tokio::time::sleep(Duration::from_secs(1)).await;
    join_set.spawn(rpc_server::serve(config.rpc));

    // block on all tasks
    while let Some(res) = join_set.join_next().await {
        // For now, if one of the components fails, crash the node
        res.unwrap().unwrap();
    }

    Ok(())
}

// MAKE GENESIS
// ===================================================================================================

pub async fn make_genesis(
    output_path: &PathBuf,
    force: &bool,
) -> anyhow::Result<()> {
    let output_file_path = Path::new(output_path);

    if !force {
        match output_file_path.try_exists() {
            Ok(file_exists) => {
                if file_exists {
                    return Err(anyhow!("Failed to generate new genesis file \"{}\" because it already exists. Use the --force flag to overwrite.", output_path.display()));
                }
            },
            Err(err) => {
                return Err(anyhow!(
                    "Failed to generate new genesis file \"{}\". Error: {err}",
                    output_path.display()
                ));
            },
        }
    }

    let faucet_key_pair = KeyPair::from_seed(&SEED_FAUCET_KEYPAIR).unwrap();
    let wallet_key_pair = KeyPair::from_seed(&SEED_WALLET_KEYPAIR).unwrap();

    let genesis_state = {
        let accounts = {
            let mut accounts_and_seeds = Vec::new();

            // fungible asset faucet
            {
                println!("Generating faucet account... ");
                let (account, seed) = create_basic_fungible_faucet(
                    SEED_FAUCET,
                    TokenSymbol::new(FUNGIBLE_FAUCET_TOKEN_SYMBOL).unwrap(),
                    FUNGIBLE_FAUCET_TOKEN_DECIMALS,
                    Felt::from(FUNGIBLE_FAUCET_TOKEN_MAX_SUPPLY),
                    AuthScheme::RpoFalcon512 {
                        pub_key: faucet_key_pair.public_key(),
                    },
                )
                .unwrap();

                println!("Done with faucet ID {}: {:?}", account.id(), seed);
                let account = AccountAndSeed { account, seed };

                accounts_and_seeds.push(account);
            }

            // basic wallet account
            {
                println!("Generating basic wallet account... ");
                let (account, seed) = create_basic_wallet(
                    SEED_WALLET,
                    AuthScheme::RpoFalcon512 {
                        pub_key: wallet_key_pair.public_key(),
                    },
                    miden_objects::accounts::AccountType::RegularAccountUpdatableCode,
                )
                .unwrap();

                let account = AccountAndSeed { account, seed };
                println!("Done with wallet: {:?}", account);

                accounts_and_seeds.push(account);
            }

            accounts_and_seeds
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
