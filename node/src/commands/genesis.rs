use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};
use miden_crypto::{dsa::rpo_falcon512::KeyPair, utils::Serializable, Felt, Word};
use miden_lib::{
    accounts::{faucets::create_basic_fungible_faucet, wallets::create_basic_wallet},
    AuthScheme,
};
use miden_node_store::genesis::GenesisState;
use miden_node_utils::config::load_config;
use miden_objects::{
    accounts::{Account, AccountType},
    assets::TokenSymbol,
};
use serde::Deserialize;

const DEFAULT_ACCOUNTS_FOLDER: &str = "accounts";

// INPUT HELPER STRUCTS
// ================================================================================================

/// *Input types are helper structures designed for parsing and deserializing configuration files.
/// They serve as intermediary representations, facilitating the conversion from
/// placeholder types (like `GenesisInput`) to internal types (like `GenesisState`).
#[derive(Debug, Deserialize)]
pub struct GenesisInput {
    pub version: u64,
    pub timestamp: u64,
    pub accounts: Vec<AccountInput>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum AccountInput {
    BasicWallet(BasicWalletInputs),
    BasicFungibleFaucet(BasicFungibleFaucetInputs),
}

#[derive(Debug, Deserialize)]
pub enum AuthSchemeInput {
    RpoFalcon512,
}

#[derive(Debug, Deserialize)]
pub struct BasicWalletInputs {
    pub mode: AccountType,
    pub seed: String,
    pub auth_scheme: AuthSchemeInput,
    pub auth_seed: String,
}

#[derive(Debug, Deserialize)]
pub struct BasicFungibleFaucetInputs {
    pub mode: AccountType,
    pub seed: String,
    pub auth_scheme: AuthSchemeInput,
    pub auth_seed: String,
    pub token_symbol: String,
    pub decimals: u8,
    pub max_supply: u64,
}

#[derive(Debug)]
pub struct AccountData {
    pub account: Account,
    pub seed: Word,
    pub auth: AuthInfo,
}

#[derive(Debug)]
pub enum AuthInfo {
    RpoFalcon512Seed([u8; 40]),
}

// MAKE GENESIS
// ===================================================================================================

pub fn make_genesis(
    output_path: &PathBuf,
    force: &bool,
    config_filepath: &PathBuf,
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

    let genesis_file_path = Path::new(config_filepath);

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

    let accounts = create_accounts(&genesis_input.accounts)?;
    println!("Accounts have successfully been created.");

    let genesis_state = GenesisState::new(accounts, genesis_input.version, genesis_input.timestamp);

    // Write genesis state as binary format
    fs::write(output_path, genesis_state.to_bytes()).unwrap_or_else(|_| {
        panic!("Failed to write genesis state to output file {}", output_path.display())
    });

    println!("Genesis initialisation successful: {} has been created", output_path.display());

    Ok(())
}

fn create_account_file(
    account: &Account,
    seed: Word,
    auth_scheme: String,
    auth_seed: String,
    index: usize,
) -> Result<()> {
    let file_path = PathBuf::from(format!("accounts/account{index}.mac"));

    let account_data = AccountData {
        account: account.clone(),
        seed,
        auth_scheme,
        auth_seed,
    };

    let json_data = serde_json::to_string(&account_data)?;

    fs::write(file_path.as_path(), json_data).map_err(|err| {
        anyhow!("Failed to write account file to {}, Error: {err}", file_path.display())
    })?;

    Ok(())
}

fn create_accounts(accounts: &[AccountInput]) -> Result<Vec<Account>> {
    // create the accounts folder
    let accounts_folder_path = PathBuf::from(DEFAULT_ACCOUNTS_FOLDER);

    fs::create_dir(accounts_folder_path.as_path())
        .map_err(|err| anyhow!("Failed to create accounts folder: {err}"))?;

    let mut final_accounts = Vec::new();

    for account in accounts {
        match account {
            AccountInput::BasicWallet(inputs) => {
                print!("Generating basic wallet account... ");
                let seed: [u8; 32] = miden_crypto::utils::hex_to_bytes(&inputs.seed)?;
                let auth_seed: [u8; 40] = miden_crypto::utils::hex_to_bytes(&inputs.auth_seed)?;

                let keypair = KeyPair::from_seed(&auth_seed)?;
                let auth_scheme = match inputs.auth_scheme {
                    AuthSchemeInput::RpoFalcon512 => AuthScheme::RpoFalcon512 {
                        pub_key: keypair.public_key(),
                    },
                };

                let (account, seed) = create_basic_wallet(seed, auth_scheme, inputs.mode)?;
                print!("done!");

                let auth_scheme_name = match inputs.auth_scheme {
                    AuthSchemeInput::RpoFalcon512 => "RpoFalcon512".to_string(),
                };

                // create account file
                create_account_file(
                    &account,
                    seed,
                    auth_scheme_name,
                    inputs.auth_seed.clone(),
                    final_accounts.len(),
                )?;

                final_accounts.push(account);
            },
            AccountInput::BasicFungibleFaucet(inputs) => {
                print!("Generating fungible faucet account... ");
                let auth_seed: [u8; 40] = miden_crypto::utils::hex_to_bytes(&inputs.auth_seed)?;
                let seed: [u8; 32] = miden_crypto::utils::hex_to_bytes(&inputs.seed)?;

                let keypair = KeyPair::from_seed(&auth_seed)?;
                let auth_scheme = match inputs.auth_scheme {
                    AuthSchemeInput::RpoFalcon512 => AuthScheme::RpoFalcon512 {
                        pub_key: keypair.public_key(),
                    },
                };

                let (account, seed) = create_basic_fungible_faucet(
                    seed,
                    TokenSymbol::try_from(inputs.token_symbol.as_str())?,
                    inputs.decimals,
                    Felt::from(inputs.max_supply),
                    auth_scheme,
                )?;
                print!("done!");

                let auth_scheme_name = match inputs.auth_scheme {
                    AuthSchemeInput::RpoFalcon512 => "RpoFalcon512".to_string(),
                };

                // create account file
                create_account_file(
                    &account,
                    seed,
                    auth_scheme_name,
                    inputs.auth_seed.clone(),
                    final_accounts.len(),
                )?;

                final_accounts.push(account);
            },
        }
    }

    Ok(final_accounts)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use figment::Jail;

    use super::make_genesis;
    use crate::DEFAULT_GENESIS_DAT_FILE_PATH;

    #[test]
    fn test_node_genesis() {
        let genesis_file_path = PathBuf::from("genesis.toml");

        Jail::expect_with(|jail| {
            jail.create_file(
                genesis_file_path.as_path(),
                r#"
                    version = 1
                    timestamp = 1672531200

                    [[accounts]]
                    type = "BasicWallet"
                    mode = "RegularAccountImmutableCode"
                    seed = "0xa123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    auth_scheme = "RpoFalcon512"
                    auth_seed = "0xb123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"

                    [[accounts]]
                    type = "BasicFungibleFaucet"
                    mode = "FungibleFaucet"
                    seed = "0xc123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    auth_scheme = "RpoFalcon512"
                    auth_seed = "0xd123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    token_symbol = "POL"
                    decimals = 12
                    max_supply = 1000000
                "#,
            )?;

            let genesis_dat_file_path = PathBuf::from(DEFAULT_GENESIS_DAT_FILE_PATH);

            //  run `make_genesis` to generate `genesis.dat` and accounts folder and files
            make_genesis(&genesis_dat_file_path, &true, &genesis_file_path).unwrap();

            // assert that the genesis.dat file exists
            assert!(genesis_dat_file_path.exists());

            let account_0_file_path = PathBuf::from("accounts/account0.mac");
            let account_1_file_path = PathBuf::from("accounts/account1.mac");

            // assert that all the account files exist
            assert!(account_0_file_path.exists());
            assert!(account_1_file_path.exists());

            Ok(())
        });
    }
}
