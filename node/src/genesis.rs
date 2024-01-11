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
use miden_node_utils::config::{hex_string_to_byte_array, load_config};
use miden_objects::{
    accounts::{Account, AccountType},
    assets::TokenSymbol,
};
use serde::Deserialize;

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

    let accounts_data = create_accounts(&genesis_input.accounts)?;
    println!("Accounts have successfully been created.");

    let accounts: Vec<Account> = accounts_data.iter().map(|(account, _)| account.clone()).collect();

    let genesis_state = GenesisState::new(accounts, genesis_input.version, genesis_input.timestamp);

    // Write genesis state as binary format
    fs::write(output_path, genesis_state.to_bytes()).unwrap_or_else(|_| {
        panic!("Failed to write genesis state to output file {}", output_path.display())
    });

    println!("Genesis initialisation successful: {} has been created", output_path.display());

    Ok(())
}

fn _create_account_files(_accounts: Vec<KeyPair>) {
    // // Write account keys
    // keypairs.into_iter().enumerate().for_each(|(index, keypair)| {
    //     let s = format!("acount{}.fsk", index);
    //     let file_path = Path::new(&s);
    //     fs::write(file_path, keypair.to_bytes())
    //         .unwrap_or_else(|_| panic!("Failed to write account file to {}", file_path.display()));
    // });
}

fn create_accounts(accounts: &[AccountInput]) -> Result<Vec<(Account, Word)>> {
    let mut final_accounts = Vec::new();

    for account in accounts {
        match account {
            AccountInput::BasicWallet(inputs) => {
                println!("Generating basic wallet account... ");
                let seed: [u8; 32] = hex_string_to_byte_array(&inputs.seed)?;
                let auth_seed: [u8; 40] = hex_string_to_byte_array(&inputs.auth_seed)?;

                let keypair = KeyPair::from_seed(&auth_seed)?;
                let auth_scheme = match inputs.auth_scheme {
                    AuthSchemeInput::RpoFalcon512 => AuthScheme::RpoFalcon512 {
                        pub_key: keypair.public_key(),
                    },
                };

                let account_pair = create_basic_wallet(seed, auth_scheme, inputs.mode)?;
                println!("Done. ");
                final_accounts.push(account_pair);
            },
            AccountInput::BasicFungibleFaucet(inputs) => {
                println!("Generating fungible faucet account... ");
                let auth_seed: [u8; 40] = hex_string_to_byte_array(&inputs.auth_seed)?;
                let seed: [u8; 32] = hex_string_to_byte_array(&inputs.seed)?;

                let keypair = KeyPair::from_seed(&auth_seed)?;
                let auth_scheme = match inputs.auth_scheme {
                    AuthSchemeInput::RpoFalcon512 => AuthScheme::RpoFalcon512 {
                        pub_key: keypair.public_key(),
                    },
                };

                let account_pair = create_basic_fungible_faucet(
                    seed,
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
            make_genesis(&genesis_dat_file_path, &true, &genesis_file_path).unwrap();
            // assert that the genesis.dat file exists
            assert!(genesis_dat_file_path.exists());

            // TODO
            // assert that all the account files exist
            Ok(())
        });
    }
}
