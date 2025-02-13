use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, bail, Context, Result};
pub use inputs::{AccountInput, AuthSchemeInput, GenesisInput};
use miden_lib::{account::faucets::create_basic_fungible_faucet, AuthScheme};
use miden_node_store::genesis::GenesisState;
use miden_node_utils::{config::load_config, crypto::get_rpo_random_coin};
use miden_objects::{
    account::{Account, AccountFile, AccountIdAnchor, AuthSecretKey},
    asset::TokenSymbol,
    crypto::{dsa::rpo_falcon512::SecretKey, utils::Serializable},
    Felt, ONE,
};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use tracing::info;

mod inputs;

const DEFAULT_ACCOUNTS_DIR: &str = "accounts/";

// MAKE GENESIS
// ================================================================================================

/// Generates a genesis file and associated account files based on a specified genesis input
///
/// # Arguments
///
/// * `output_path` - A `PathBuf` reference to the path where the genesis file will be created.
/// * `force` - A boolean flag to determine if an existing genesis file should be overwritten.
/// * `inputs_path` - A `PathBuf` reference to the genesis inputs file's path.
///
/// # Returns
///
/// This function returns a `Result` type. On successful creation of the genesis file, it returns
/// `Ok(())`. If it fails at any point, due to issues like file existence checks or read/write
/// operations, it returns an `Err` with a detailed error message.
pub fn make_genesis(inputs_path: &PathBuf, output_path: &PathBuf, force: bool) -> Result<()> {
    let inputs_path = Path::new(inputs_path);
    let output_path = Path::new(output_path);

    if !force {
        if let Ok(file_exists) = output_path.try_exists() {
            if file_exists {
                return Err(anyhow!("Failed to generate new genesis file {} because it already exists. Use the --force flag to overwrite.", output_path.display()));
            }
        } else {
            return Err(anyhow!("Failed to open {} file.", output_path.display()));
        }
    }

    if let Ok(file_exists) = inputs_path.try_exists() {
        if !file_exists {
            return Err(anyhow!(
                "The {} file does not exist. It is necessary to generate the genesis file. Use the --inputs-path flag to pass in the genesis input file.",
                inputs_path.display()
            ));
        }
    } else {
        return Err(anyhow!("Failed to open {} file.", inputs_path.display()));
    }

    let Some(parent_path) = output_path.parent() else {
        anyhow::bail!("There has been an error processing output_path: {}", output_path.display());
    };

    let genesis_input: GenesisInput = load_config(inputs_path).map_err(|err| {
        anyhow!("Failed to load {} genesis input file: {err}", inputs_path.display())
    })?;
    info!("Genesis input file: {} has successfully been loaded.", inputs_path.display());

    let accounts_path = parent_path.join(DEFAULT_ACCOUNTS_DIR);
    let accounts =
        create_accounts(&genesis_input.accounts.unwrap_or_default(), &accounts_path, force)?;

    let genesis_state = GenesisState::new(accounts, genesis_input.version, genesis_input.timestamp);
    fs::write(output_path, genesis_state.to_bytes()).unwrap_or_else(|_| {
        panic!("Failed to write genesis state to output file {}", output_path.display())
    });
    info!("Miden node genesis successful: {} has been created", output_path.display());

    Ok(())
}

/// Converts the provided list of account inputs into [Account] objects.
///
/// This function also writes the account data files into the default accounts directory.
fn create_accounts(
    accounts: &[AccountInput],
    accounts_path: impl AsRef<Path>,
    force: bool,
) -> Result<Vec<Account>> {
    if accounts_path.as_ref().try_exists()? {
        if !force {
            bail!(
                "Failed to create accounts directory because it already exists. \
                Use the --force flag to overwrite."
            );
        }
        fs::remove_dir_all(&accounts_path).context("Failed to remove accounts directory")?;
    }

    fs::create_dir_all(&accounts_path).context("Failed to create accounts directory")?;

    let mut final_accounts = Vec::new();
    let mut faucet_count = 0;
    let mut rng = ChaCha20Rng::from_seed(rand::random());

    for account in accounts {
        // build account data from account inputs
        let (mut account_data, name) = match account {
            AccountInput::BasicFungibleFaucet(inputs) => {
                info!("Creating fungible faucet account...");
                let (auth_scheme, auth_secret_key) = gen_auth_keys(inputs.auth_scheme, &mut rng);

                let storage_mode = inputs.storage_mode.as_str().try_into()?;
                let (account, account_seed) = create_basic_fungible_faucet(
                    rng.gen(),
                    AccountIdAnchor::PRE_GENESIS,
                    TokenSymbol::try_from(inputs.token_symbol.as_str())?,
                    inputs.decimals,
                    Felt::try_from(inputs.max_supply)
                        .expect("max supply value is greater than or equal to the field modulus"),
                    storage_mode,
                    auth_scheme,
                )?;

                let name = format!(
                    "faucet{}",
                    (faucet_count > 0).then(|| faucet_count.to_string()).unwrap_or_default()
                );
                faucet_count += 1;

                (AccountFile::new(account, Some(account_seed), auth_secret_key), name)
            },
        };

        // write account data to file
        let path = accounts_path.as_ref().join(format!("{name}.mac"));

        if !force && matches!(path.try_exists(), Ok(true)) {
            bail!("Failed to generate account file {} because it already exists. Use the --force flag to overwrite.", path.display());
        }

        account_data.account.set_nonce(ONE)?;

        account_data.write(&path)?;

        info!("Account \"{name}\" has successfully been saved to: {}", path.display());

        final_accounts.push(account_data.account);
    }

    Ok(final_accounts)
}

fn gen_auth_keys(
    auth_scheme_input: AuthSchemeInput,
    rng: &mut ChaCha20Rng,
) -> (AuthScheme, AuthSecretKey) {
    match auth_scheme_input {
        AuthSchemeInput::RpoFalcon512 => {
            let secret = SecretKey::with_rng(&mut get_rpo_random_coin(rng));

            (
                AuthScheme::RpoFalcon512 { pub_key: secret.public_key() },
                AuthSecretKey::RpoFalcon512(secret),
            )
        },
    }
}

// TESTS
// ================================================================================================

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use figment::Jail;
    use miden_node_store::genesis::GenesisState;
    use miden_objects::{account::AccountFile, utils::serde::Deserializable};

    use crate::DEFAULT_GENESIS_FILE_PATH;

    #[test]
    fn make_genesis() {
        let genesis_inputs_file_path = PathBuf::from("genesis.toml");

        // node genesis configuration
        Jail::expect_with(|jail| {
            jail.create_file(
                genesis_inputs_file_path.as_path(),
                r#"
                version = 1
                timestamp = 1672531200

                [[accounts]]
                type = "BasicFungibleFaucet"
                auth_scheme = "RpoFalcon512"
                token_symbol = "POL"
                decimals = 12
                max_supply = 1000000
                storage_mode = "public"
            "#,
            )?;

            let genesis_dat_file_path = PathBuf::from(DEFAULT_GENESIS_FILE_PATH);

            //  run make_genesis to generate genesis.dat and accounts folder and files
            super::make_genesis(&genesis_inputs_file_path, &genesis_dat_file_path, true).unwrap();

            let a0_file_path = PathBuf::from("accounts/faucet.mac");

            // assert that the genesis.dat and account files exist
            assert!(genesis_dat_file_path.exists());
            assert!(a0_file_path.exists());

            // deserialize account and genesis_state
            let a0 = AccountFile::read(a0_file_path).unwrap();

            // assert that the account has the corresponding storage mode
            assert!(a0.account.is_public());

            let genesis_file_contents = fs::read(genesis_dat_file_path).unwrap();
            let genesis_state = GenesisState::read_from_bytes(&genesis_file_contents).unwrap();

            // build supposed genesis_state
            let supposed_genesis_state = GenesisState::new(vec![a0.account], 1, 1_672_531_200);

            // assert that both genesis_state(s) are eq
            assert_eq!(genesis_state, supposed_genesis_state);

            Ok(())
        });
    }
}
