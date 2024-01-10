use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
    u8,
};

use anyhow::{anyhow, Result};
use miden_crypto::{dsa::rpo_falcon512::KeyPair, utils::Serializable, Felt, Word};
use miden_lib::{
    accounts::{faucets::create_basic_fungible_faucet, wallets::create_basic_wallet},
    AuthScheme,
};
use miden_node_block_producer::server as block_producer_server;
use miden_node_rpc::server as rpc_server;
use miden_node_store::{
    db::Db,
    genesis::{AccountInput, AuthSchemeInput, GenesisInput, GenesisState},
    server as store_server,
};
use miden_node_utils::config::load_config;
use miden_objects::{accounts::Account, assets::TokenSymbol};
use tokio::task::JoinSet;

use crate::{config::NodeTopLevelConfig, INPUT_GENESIS_FILE_PATH};

// START
// ===================================================================================================

pub async fn start(config_filepath: &Path) -> Result<()> {
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
) -> Result<()> {
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

    let genesis_file_path = Path::new(INPUT_GENESIS_FILE_PATH);

    if let Ok(file_exists) = genesis_file_path.try_exists() {
        if !file_exists {
            return Err(anyhow!(
                "The {} file does not exist. It is necessary to initialise the node GenesisState.",
                genesis_file_path.display()
            ));
        }
    } else {
        return Err(anyhow!("Failed to open process {} file.", genesis_file_path.display()));
    }

    let genesis_input: GenesisInput = load_config(genesis_file_path).extract().map_err(|err| {
        anyhow!("Failed to load {} config file: {err}", genesis_file_path.display())
    })?;
    println!("Config file: {} has successfully been loaded.", genesis_file_path.display());

    let accounts_data = create_accounts(&genesis_input.accounts)?;
    println!("Accounts have successfully been created.");

    let accounts: Vec<Account> = accounts_data.iter().map(|(account, _)| account.clone()).collect();

    let genesis_state = GenesisState::new(accounts, genesis_input.version, genesis_input.timestamp);

    println!("GenesisState: {:#?}", genesis_state);

    Ok(())
}

fn _generate_files(
    genesis_state: GenesisState,
    genesis_state_path: &Path,
    keypairs: Vec<KeyPair>,
) {
    // Write genesis state as binary format
    fs::write(genesis_state_path, genesis_state.to_bytes()).unwrap_or_else(|_| {
        panic!("Failed to write genesis state to output file {}", genesis_state_path.display())
    });

    // Write account keys
    keypairs.into_iter().enumerate().for_each(|(index, keypair)| {
        let s = format!("acount{}.fsk", index);
        let file_path = Path::new(&s);
        fs::write(file_path, keypair.to_bytes())
            .unwrap_or_else(|_| panic!("Failed to write account file to {}", file_path.display()));
    });
}

fn create_accounts(accounts: &[AccountInput]) -> anyhow::Result<Vec<(Account, Word)>> {
    let mut final_accounts = Vec::new();

    for account in accounts {
        match account {
            AccountInput::BasicWallet(inputs) => {
                println!("Generating basic wallet account... ");
                let account_seed: [u8; 32] = hex_string_to_byte_array(&inputs.account_seed)?;
                let auth_seed: [u8; 40] = hex_string_to_byte_array(&inputs.auth_seed)?;

                let keypair = KeyPair::from_seed(&auth_seed)?;
                let auth_scheme = match inputs.auth_scheme {
                    AuthSchemeInput::RpoFalcon512 => AuthScheme::RpoFalcon512 {
                        pub_key: keypair.public_key(),
                    },
                };

                let account_pair = create_basic_wallet(account_seed, auth_scheme, inputs.mode)?;
                println!("Done. ");
                final_accounts.push(account_pair);
            },
            AccountInput::BasicFungibleFaucet(inputs) => {
                println!("Generating fungible faucet account... ");
                let auth_seed: [u8; 40] = hex_string_to_byte_array(&inputs.auth_seed)?;
                let account_seed: [u8; 32] = hex_string_to_byte_array(&inputs.account_seed)?;

                let keypair = KeyPair::from_seed(&auth_seed)?;
                let auth_scheme = match inputs.auth_scheme {
                    AuthSchemeInput::RpoFalcon512 => AuthScheme::RpoFalcon512 {
                        pub_key: keypair.public_key(),
                    },
                };

                let account_pair = create_basic_fungible_faucet(
                    account_seed,
                    TokenSymbol::try_from(inputs.token_symbol.as_str())?,
                    inputs.decimals,
                    Felt::from(inputs.max_supply),
                    auth_scheme,
                )?;
                println!("Done.");
                final_accounts.push(account_pair);
            },
        }
    }

    Ok(final_accounts)
}

fn hex_string_to_byte_array<const N: usize>(hex_str: &str) -> anyhow::Result<[u8; N]> {
    let mut bytes_array = [0u8; N];
    hex::decode_to_slice(hex_str, &mut bytes_array)?;
    Ok(bytes_array)
}
