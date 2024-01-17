use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};
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
pub struct BasicWalletInputs {
    pub mode: AccountType,
    pub init_seed: String,
    pub auth_scheme: AuthSchemeInput,
    pub auth_seed: String,
}

#[derive(Debug, Deserialize)]
pub struct BasicFungibleFaucetInputs {
    pub mode: AccountType,
    pub init_seed: String,
    pub auth_scheme: AuthSchemeInput,
    pub auth_seed: String,
    pub token_symbol: String,
    pub decimals: u8,
    pub max_supply: u64,
}

#[derive(Debug, Deserialize)]
pub enum AuthSchemeInput {
    RpoFalcon512,
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
    account_seed: Option<Word>,
    auth_info: AuthData,
    index: usize,
) -> Result<()> {
    let account_data = AccountData::new(account.clone(), account_seed, auth_info);

    let path = format!("accounts/account{}.mac", index);
    let filepath = Path::new(&path);

    account_data.write(&filepath)?;

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
                print!("done!");

                let auth_info = match inputs.auth_scheme {
                    AuthSchemeInput::RpoFalcon512 => AuthData::RpoFalcon512Seed(auth_seed),
                };

                // create account file
                create_account_file(&account, Some(account_seed), auth_info, final_accounts.len())?;

                final_accounts.push(account);
            },
            AccountInput::BasicFungibleFaucet(inputs) => {
                print!("Generating fungible faucet account... ");
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
                print!("done!");

                let auth_info = match inputs.auth_scheme {
                    AuthSchemeInput::RpoFalcon512 => AuthData::RpoFalcon512Seed(auth_seed),
                };

                // create account file
                create_account_file(&account, Some(account_seed), auth_info, final_accounts.len())?;

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
    use miden_crypto::{dsa::rpo_falcon512::KeyPair, utils::hex_to_bytes};
    use miden_lib::{
        accounts::{faucets::create_basic_fungible_faucet, wallets::create_basic_wallet},
        transaction::TransactionKernel,
        AuthScheme,
    };
    use miden_objects::{
        accounts::{
            get_account_seed, Account, AccountCode, AccountId, AccountStorage, AccountType,
            ACCOUNT_ID_REGULAR_ACCOUNT_IMMUTABLE_CODE_ON_CHAIN,
        },
        assembly::ModuleAst,
        assets::{AssetVault, TokenSymbol},
        Felt,
    };

    use super::make_genesis;
    use crate::{
        commands::genesis::{AccountData, AuthData},
        DEFAULT_GENESIS_DAT_FILE_PATH,
    };

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

            //  run `make_genesis` to generate `genesis.dat` and accounts folder and files
            make_genesis(&genesis_dat_file_path, &true, &genesis_file_path).unwrap();

            // assert that the genesis.dat file exists
            assert!(genesis_dat_file_path.exists());

            let a0_file_path = PathBuf::from("accounts/account0.mac");
            let a1_file_path = PathBuf::from("accounts/account1.mac");

            // assert that all the account files exist
            assert!(a0_file_path.exists());
            assert!(a1_file_path.exists());

            // deserialise the `AccountData` from the 2 created account files
            let account_data_0 = AccountData::read(a0_file_path.as_path()).unwrap();
            let account_data_1 = AccountData::read(a1_file_path.as_path()).unwrap();

            let a0_init_seed =
                hex_to_bytes("0xa123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
                    .unwrap();

            let a0_auth_seed= hex_to_bytes("0xb123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef").unwrap();

            let a0_type = AccountType::RegularAccountImmutableCode;

            let a0_keypair = KeyPair::from_seed(&a0_auth_seed).unwrap();

            let a0_auth_scheme = AuthScheme::RpoFalcon512 {
                pub_key: a0_keypair.public_key(),
            };

            let (a0, seed_0) = create_basic_wallet(a0_init_seed, a0_auth_scheme, a0_type).unwrap();

            let a1_init_seed =
                hex_to_bytes("0xc123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
                    .unwrap();

            let a1_auth_seed = hex_to_bytes("0xd123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef").unwrap();

            let a1_keypair = KeyPair::from_seed(&a1_auth_seed).unwrap();

            let a1_auth_scheme = AuthScheme::RpoFalcon512 {
                pub_key: a1_keypair.public_key(),
            };

            let (a1, seed_1) = create_basic_fungible_faucet(
                a1_init_seed,
                TokenSymbol::try_from("POL").unwrap(),
                12,
                Felt::new(1000000),
                a1_auth_scheme,
            )
            .unwrap();

            let account_id = AccountId::new(
                seed_0,
                account_data_0.account.code().root(),
                account_data_0.account.storage().root(),
            )
            .unwrap();

            // // Assert that both the deserialized and created `AccountData` are eq
            // assert_eq!(
            //     account_data_0,
            //     AccountData::new(
            //         a0.clone(),
            //         Some(seed_0),
            //         AuthInfo::RpoFalcon512Seed(a0_auth_seed)
            //     )
            // );

            // // Assert that both the deserialized and created `AccountData` are eq
            // assert_eq!(
            //     account_data_1,
            //     AccountData::new(a1, Some(seed_1), AuthInfo::RpoFalcon512Seed(a1_auth_seed))
            // );

            Ok(())
        });
    }

    fn configuration_file_creates_correct_struct() {
        let genesis_file_path = PathBuf::from("genesis.toml");
    }

    // fn accountdata_is_correctly_serialised_deserialised() {
    //     // Setup
    //     let assembler = TransactionKernel::assembler();
    //     // let id = AccountId::try_from(ACCOUNT_ID_REGULAR_ACCOUNT_IMMUTABLE_CODE_ON_CHAIN).unwrap();
    //     let (id , seed) = get_account_seed(init_seed, account_type, on_chain, code_root, storage_root)
    //     let vault = AssetVault::new(&[]).unwrap();
    //     let storage = AccountStorage::new(vec![]).unwrap();
    //     let mast = ModuleAst::new(vec![], vec![], None).unwrap();
    //     let code = AccountCode::new(mast, &assembler).unwrap();
    //     let nonce = Felt::new(0);
    //     let account = Account::new(id, vault, storage, code, nonce);
    //     let account_data = AccountData::new(account, account., auth)
    // }
}
