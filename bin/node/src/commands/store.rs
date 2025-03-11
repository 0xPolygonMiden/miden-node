use std::{
    fs::File,
    io::Write,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use miden_lib::{account::faucets::create_basic_fungible_faucet, utils::Serializable, AuthScheme};
use miden_node_store::{genesis::GenesisState, server::Store};
use miden_node_utils::{crypto::get_rpo_random_coin, grpc::UrlExt};
use miden_objects::{
    account::{AccountFile, AccountIdAnchor, AuthSecretKey},
    asset::TokenSymbol,
    crypto::dsa::rpo_falcon512::SecretKey,
    Felt, ONE,
};
use rand::{Rng, SeedableRng};
use rand_chacha::{ChaCha20Rng, ChaChaRng};
use serde::{Deserialize, Serialize};
use url::Url;

use super::{ENV_ENABLE_OTEL, ENV_STORE_DIRECTORY, ENV_STORE_URL};

#[derive(clap::Subcommand)]
pub enum StoreCommand {
    /// Dumps the default genesis configuration to stdout.
    ///
    /// Use this as a starting point to modify the genesis data for `bootstrap`.
    DumpGenesis,

    /// Bootstraps the blockchain database with the genesis block.
    ///
    /// This populates the genesis block's data with the accounts and data listed in the
    /// configuration file.
    ///
    /// Each generated genesis account's data is also written to disk. This includes the private
    /// key which can be used to create transactions for these accounts.
    ///
    /// See also: `dump-genesis`
    Bootstrap {
        /// Genesis configuration file.
        ///
        /// If not provided the default configuration is used.
        config: Option<PathBuf>,
        /// Directory in which to store the database and raw block data.
        #[arg(env = ENV_STORE_DIRECTORY)]
        data_directory: PathBuf,
        // Directory to write the account data to.
        accounts_directory: PathBuf,
    },

    /// Starts the store component.
    Start {
        /// Url at which to serve the gRPC API.
        #[arg(env = ENV_STORE_URL)]
        url: Url,

        /// Directory in which to store the database and raw block data.
        #[arg(env = ENV_STORE_DIRECTORY)]
        data_directory: PathBuf,

        /// Enables the exporting of traces for OpenTelemetry.
        ///
        /// This can be further configured using environment variables as defined in the official
        /// OpenTelemetry documentation. See our operator manual for further details.
        #[arg(long = "open-telemetry", default_value_t = false, env = ENV_ENABLE_OTEL)]
        open_telemetry: bool,
    },
}

impl StoreCommand {
    /// Executes the subcommand as described by each variants documentation.
    pub async fn handle(self) -> anyhow::Result<()> {
        match self {
            StoreCommand::DumpGenesis => Self::dump_default_genesis(),
            StoreCommand::Bootstrap {
                config,
                data_directory,
                accounts_directory,
            } => Self::bootstrap(config, data_directory, accounts_directory),
            // Note: open-telemetry is handled in main.
            StoreCommand::Start { url, data_directory, open_telemetry: _ } => {
                Self::start(url, data_directory).await
            },
        }
    }

    pub fn is_open_telemetry_enabled(&self) -> bool {
        if let Self::Start { open_telemetry, .. } = self {
            *open_telemetry
        } else {
            false
        }
    }

    async fn start(url: Url, data_directory: PathBuf) -> anyhow::Result<()> {
        let listener =
            url.to_socket().context("Failed to extract socket address from store URL")?;
        let listener = tokio::net::TcpListener::bind(listener)
            .await
            .context("Failed to bind to store's gRPC URL")?;

        Store::init(listener, data_directory)
            .await
            .context("Loading store")?
            .serve()
            .await
            .context("Serving store")
    }

    fn dump_default_genesis() -> anyhow::Result<()> {
        let to_dump = toml::to_string(&GenesisConfig::default())
            .context("failed to serialize the default genesis configuration")?;

        println!("{to_dump}");
        Ok(())
    }

    fn bootstrap(
        genesis_input: Option<PathBuf>,
        data_directory: PathBuf,
        accounts_directory: PathBuf,
    ) -> anyhow::Result<()> {
        // Parse the genesis configuration input.
        let input = if let Some(genesis_input) = genesis_input {
            let input = std::fs::read_to_string(&genesis_input).with_context(|| {
                format!("failed to read genesis configuration from {}", genesis_input.display())
            })?;
            toml::from_str(&input).context("failed to parse genesis configuration file")?
        } else {
            GenesisConfig::default()
        };
        let GenesisConfig { version, timestamp, accounts } = input;

        // Generate the accounts.
        let mut rng = ChaCha20Rng::from_seed(rand::random());
        let n_accounts = accounts.as_ref().map(Vec::len).unwrap_or_default();
        let accounts = accounts
            .into_iter()
            .flatten()
            .enumerate()
            .inspect(|(idx, _)| tracing::info!(index=%idx, total=n_accounts, "Generating account"))
            .map(|(idx, input)| {
                Self::generate_account(input, &mut rng)
                    .with_context(|| format!("failed to generate account {idx}"))
            })
            .collect::<Result<Vec<AccountFile>, _>>()
            .context("failed to generate accounts")?;

        // Write account data to disk (including secrets).
        //
        // Without this private accounts would be inaccessible by the user.
        // This is not used directly by the node, but rather by the owner / operator of the node.
        for (idx, account) in accounts.iter().enumerate() {
            let filepath = accounts_directory.join(format!("account_{idx}.mac"));
            File::create_new(&filepath)
                .and_then(|mut file| file.write_all(&account.to_bytes()))
                .with_context(|| {
                    format!("failed to write data for account {idx} to file {}", filepath.display())
                })?;
        }

        // Write the genesis state to disk. This is used to seed the database's genesis block.
        let accounts = accounts.into_iter().map(|account| account.account).collect();
        let genesis_state = GenesisState::new(accounts, version, timestamp);
        let genesis_output = data_directory.join(miden_node_store::GENESIS_STATE_FILENAME);
        File::create_new(&genesis_output)
            .and_then(|mut file| file.write_all(&genesis_state.to_bytes()))
            .with_context(|| {
                format!("failed to write genesis data to file {}", genesis_output.display())
            })
    }

    fn generate_account(
        input: AccountInput,
        mut rng: &mut ChaChaRng,
    ) -> anyhow::Result<AccountFile> {
        let AccountInput::BasicFungibleFaucet(input) = input;

        let (auth_scheme, auth_secret_key) = input.auth_scheme.gen_auth_keys(&mut rng);

        let storage_mode = input.storage_mode.as_str().try_into()?;
        let (mut account, account_seed) = create_basic_fungible_faucet(
            rng.r#gen(),
            AccountIdAnchor::PRE_GENESIS,
            TokenSymbol::try_from(input.token_symbol.as_str())?,
            input.decimals,
            Felt::try_from(input.max_supply)
                .map_err(|err| anyhow::anyhow!("{err}"))
                .context("failed to parse max supply")?,
            storage_mode,
            auth_scheme,
        )?;

        // TODO: why do we do this?
        account.set_nonce(ONE).context("failed to set account nonce to 1")?;

        Ok(AccountFile::new(account, Some(account_seed), auth_secret_key))
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GenesisConfig {
    pub version: u32,
    pub timestamp: u32,
    pub accounts: Option<Vec<AccountInput>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum AccountInput {
    BasicFungibleFaucet(BasicFungibleFaucetInputs),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BasicFungibleFaucetInputs {
    pub auth_scheme: AuthSchemeInput,
    pub token_symbol: String,
    pub decimals: u8,
    pub max_supply: u64,
    pub storage_mode: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub enum AuthSchemeInput {
    RpoFalcon512,
}

impl AuthSchemeInput {
    pub fn gen_auth_keys(&self, rng: &mut ChaCha20Rng) -> (AuthScheme, AuthSecretKey) {
        match self {
            AuthSchemeInput::RpoFalcon512 => {
                let secret = SecretKey::with_rng(&mut get_rpo_random_coin(rng));

                (
                    AuthScheme::RpoFalcon512 { pub_key: secret.public_key() },
                    AuthSecretKey::RpoFalcon512(secret),
                )
            },
        }
    }
}

impl Default for GenesisConfig {
    fn default() -> Self {
        Self {
            version: 1,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Current timestamp should be greater than unix epoch")
                .as_secs() as u32,
            accounts: Some(vec![AccountInput::BasicFungibleFaucet(BasicFungibleFaucetInputs {
                auth_scheme: AuthSchemeInput::RpoFalcon512,
                token_symbol: "POL".to_string(),
                decimals: 12,
                max_supply: 1_000_000,
                storage_mode: "public".to_string(),
            })]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ensures that [`GenesisInput::default()`] is serializable since otherwise we panic in the
    /// dump command.
    #[tokio::test]
    async fn dump_config_succeeds() {
        StoreCommand::DumpGenesis.handle().await.unwrap();
    }
}
