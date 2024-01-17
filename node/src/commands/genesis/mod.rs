use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};
use input::{AccountInput, AuthSchemeInput, GenesisInput};
use miden_crypto::{
    dsa::rpo_falcon512::KeyPair,
    utils::{hex_to_bytes, Serializable},
};
use miden_lib::{
    accounts::{faucets::create_basic_fungible_faucet, wallets::create_basic_wallet},
    AuthScheme,
};
use miden_node_store::genesis::GenesisState;
use miden_node_utils::config::load_config;
use miden_objects::{
    accounts::{Account, AccountData, AccountType, AuthData},
    assets::TokenSymbol,
    Felt, Word,
};

mod input;

const DEFAULT_ACCOUNTS_FOLDER: &str = "accounts";

// MAKE GENESIS
// ===================================================================================================

/// Generates a genesis file and associated account files based on a specified configuration.
///
/// This function creates a new genesis file and associated account files at the specified output paths.
/// It checks for the existence of the file, and if it already exists, an error is thrown
/// unless the `force` flag is set to overwrite it. The function also verifies the existence
/// of a configuration file required for initializing the genesis file.
///
/// # Arguments
///
/// * `output_path` - A `PathBuf` reference to the path where the genesis file will be created.
/// * `force` - A boolean flag to determine if an existing genesis file should be overwritten.
/// * `config_filepath` - A `PathBuf` reference to the configuration file's path.
///
/// # Returns
///
/// This function returns a `Result` type. On successful creation of the genesis file, it returns `Ok(())`.
/// If it fails at any point, due to issues like file existence checks or read/write operations, it returns an `Err` with a detailed error message.
pub fn make_genesis(
    output_path: &PathBuf,
    force: &bool,
    config_filepath: &PathBuf,
) -> Result<()> {
    let output_file_path = Path::new(output_path);

    if !force {
        if let Ok(file_exists) = output_file_path.try_exists() {
            if file_exists {
                return Err(anyhow!("Failed to generate new genesis file {} because it already exists. Use the --force flag to overwrite.", output_path.display()));
            }
        } else {
            return Err(anyhow!("Failed to open {} file.", output_file_path.display()));
        }
    }

    let genesis_file_path = Path::new(config_filepath);

    if let Ok(file_exists) = genesis_file_path.try_exists() {
        if !file_exists {
            return Err(anyhow!(
                "The {} file does not exist. It is necessary to initialise the node",
                genesis_file_path.display()
            ));
        }
    } else {
        return Err(anyhow!("Failed to open {} file.", genesis_file_path.display()));
    }

    let genesis_input: GenesisInput = load_config(genesis_file_path).extract().map_err(|err| {
        anyhow!("Failed to load {} config file: {err}", genesis_file_path.display())
    })?;

    println!("Config file: {} has successfully been loaded.", genesis_file_path.display());

    let accounts = create_accounts(&genesis_input.accounts)?;

    println!("Accounts have successfully been created at: /{}", DEFAULT_ACCOUNTS_FOLDER);

    let genesis_state = GenesisState::new(accounts, genesis_input.version, genesis_input.timestamp);

    fs::write(output_path, genesis_state.to_bytes()).unwrap_or_else(|_| {
        panic!("Failed to write genesis state to output file {}", output_path.display())
    });

    println!("Node genesis successful: {} has been created", output_path.display());

    Ok(())
}

/// Serializes and Writes AccountData to a File.
///
/// This function is a utility within the account creation process, specifically used by the `create_accounts` function.
/// It takes an account instance along with its seed and authentication data, serializes this information into an `AccountData`
/// object, and writes it to a uniquely named file. The file naming convention uses an index to ensure uniqueness and is stored
/// within a dedicated 'accounts' directory.
fn create_account_file(
    account: Account,
    account_seed: Option<Word>,
    auth_info: AuthData,
    index: usize,
) -> Result<()> {
    let account_data = AccountData::new(account, account_seed, auth_info);

    let path = format!("accounts/account{}.mac", index);
    let filepath = Path::new(&path);

    account_data.write(filepath)?;

    Ok(())
}

/// Create acccounts deserialised from genesis configuration file
///
/// This function is used by the `make_genesis` function during the genesis phase of the node.
/// It enables the creation of the accounts that have been deserialised from the configuration file
/// used for the node initialisation.
fn create_accounts(accounts: &[AccountInput]) -> Result<Vec<Account>> {
    let accounts_folder_path = PathBuf::from(DEFAULT_ACCOUNTS_FOLDER);

    fs::create_dir(accounts_folder_path.as_path())
        .map_err(|err| anyhow!("Failed to create accounts folder: {err}"))?;

    let mut final_accounts = Vec::new();

    for account in accounts {
        match account {
            AccountInput::BasicWallet(inputs) => {
                print!("Creating basic wallet account...");
                let init_seed = hex_to_bytes(&inputs.init_seed)?;
                let auth_seed = hex_to_bytes(&inputs.auth_seed)?;

                let keypair = KeyPair::from_seed(&auth_seed)?;
                let auth_scheme = match inputs.auth_scheme {
                    AuthSchemeInput::RpoFalcon512 => AuthScheme::RpoFalcon512 {
                        pub_key: keypair.public_key(),
                    },
                };

                let (account, account_seed) = create_basic_wallet(
                    init_seed,
                    auth_scheme,
                    AccountType::RegularAccountImmutableCode,
                )?;

                let auth_info = match inputs.auth_scheme {
                    AuthSchemeInput::RpoFalcon512 => AuthData::RpoFalcon512Seed(auth_seed),
                };

                create_account_file(
                    account.clone(),
                    Some(account_seed),
                    auth_info,
                    final_accounts.len(),
                )?;

                final_accounts.push(account);
            },
            AccountInput::BasicFungibleFaucet(inputs) => {
                println!("Creating fungible faucet account...");
                let init_seed = hex_to_bytes(&inputs.init_seed)?;
                let auth_seed = hex_to_bytes(&inputs.auth_seed)?;

                let keypair = KeyPair::from_seed(&auth_seed)?;
                let auth_scheme = match inputs.auth_scheme {
                    AuthSchemeInput::RpoFalcon512 => AuthScheme::RpoFalcon512 {
                        pub_key: keypair.public_key(),
                    },
                };

                let (account, account_seed) = create_basic_fungible_faucet(
                    init_seed,
                    TokenSymbol::try_from(inputs.token_symbol.as_str())?,
                    inputs.decimals,
                    Felt::from(inputs.max_supply),
                    auth_scheme,
                )?;

                let auth_info = match inputs.auth_scheme {
                    AuthSchemeInput::RpoFalcon512 => AuthData::RpoFalcon512Seed(auth_seed),
                };

                create_account_file(
                    account.clone(),
                    Some(account_seed),
                    auth_info,
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
    use std::{fs, path::PathBuf};

    use figment::Jail;
    use miden_crypto::utils::Deserializable;
    use miden_node_store::genesis::GenesisState;
    use miden_objects::accounts::AccountData;

    use super::make_genesis;
    use crate::DEFAULT_GENESIS_DAT_FILE_PATH;

    #[test]
    fn test_node_genesis() {
        let genesis_file_path = PathBuf::from("genesis.toml");

        // node genesis configuration
        Jail::expect_with(|jail| {
            jail.create_file(
                genesis_file_path.as_path(),
                r#"
                version = 1
                timestamp = 1672531200

                [[accounts]]
                type = "BasicWallet"
                mode = "RegularAccountImmutableCode"
                init_seed = "0xa123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                auth_scheme = "RpoFalcon512"
                auth_seed = "0xb123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"

                [[accounts]]
                type = "BasicFungibleFaucet"
                mode = "FungibleFaucet"
                init_seed = "0xc123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                auth_scheme = "RpoFalcon512"
                auth_seed = "0xd123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                token_symbol = "POL"
                decimals = 12
                max_supply = 1000000
            "#,
            )?;

            let genesis_dat_file_path = PathBuf::from(DEFAULT_GENESIS_DAT_FILE_PATH);

            //  run make_genesis to generate genesis.dat and accounts folder and files
            make_genesis(&genesis_dat_file_path, &true, &genesis_file_path).unwrap();

            let a0_file_path = PathBuf::from("accounts/account0.mac");
            let a1_file_path = PathBuf::from("accounts/account1.mac");

            // assert that the genesis.dat and account files exist
            assert!(genesis_dat_file_path.exists());
            assert!(a0_file_path.exists());
            assert!(a1_file_path.exists());

            // deserialise accounts and genesis_state
            let a0 = AccountData::read(a0_file_path).unwrap();
            let a1 = AccountData::read(a1_file_path).unwrap();

            let genesis_file_contents = fs::read(genesis_dat_file_path).unwrap();
            let genesis_state = GenesisState::read_from_bytes(&genesis_file_contents).unwrap();

            // build supposed genesis_state
            let supposed_genesis_state =
                GenesisState::new(vec![a0.account, a1.account], 1, 1672531200);

            // assert that both genesis_state(s) are eq
            assert_eq!(genesis_state, supposed_genesis_state);

            Ok(())
        });
    }
}
