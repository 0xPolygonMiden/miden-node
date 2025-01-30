mod client;
mod config;
mod errors;
mod handlers;
mod state;
mod store;

use std::path::PathBuf;

use anyhow::Context;
use axum::{
    routing::{get, post},
    Router,
};
use clap::{Parser, Subcommand};
use client::initialize_faucet_client;
use handlers::{get_index_css, get_index_html, get_index_js};
use http::HeaderValue;
use miden_lib::{account::faucets::create_basic_fungible_faucet, AuthScheme};
use miden_node_utils::{config::load_config, crypto::get_rpo_random_coin, version::LongVersion};
use miden_objects::{
    account::{AccountData, AccountStorageMode, AuthSecretKey},
    asset::TokenSymbol,
    crypto::dsa::rpo_falcon512::SecretKey,
    Felt,
};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use state::FaucetState;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, set_header::SetResponseHeaderLayer, trace::TraceLayer};
use tracing::info;

use crate::{
    config::{FaucetConfig, DEFAULT_FAUCET_ACCOUNT_PATH},
    handlers::{get_metadata, get_tokens},
};

// CONSTANTS
// =================================================================================================

const COMPONENT: &str = "miden-faucet";
const FAUCET_CONFIG_FILE_PATH: &str = "miden-faucet.toml";

// COMMANDS
// ================================================================================================

#[derive(Parser)]
#[command(version, about, long_about = None, long_version = long_version().to_string())]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Start the faucet server
    Start {
        #[arg(short, long, value_name = "FILE", default_value = FAUCET_CONFIG_FILE_PATH)]
        config: PathBuf,
    },

    /// Create a new public faucet account and save to the specified file
    CreateFaucetAccount {
        #[arg(short, long, value_name = "FILE", default_value = FAUCET_CONFIG_FILE_PATH)]
        config_path: PathBuf,
        #[arg(short, long, value_name = "FILE", default_value = DEFAULT_FAUCET_ACCOUNT_PATH)]
        output_path: PathBuf,
        #[arg(short, long)]
        token_symbol: String,
        #[arg(short, long)]
        decimals: u8,
        #[arg(short, long)]
        max_supply: u64,
    },

    /// Generate default configuration file for the faucet
    Init {
        #[arg(short, long, default_value = FAUCET_CONFIG_FILE_PATH)]
        config_path: String,
        #[arg(short, long, default_value = DEFAULT_FAUCET_ACCOUNT_PATH)]
        faucet_account_path: String,
    },
}

// MAIN
// =================================================================================================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    miden_node_utils::logging::setup_logging().context("Failed to initialize logging")?;

    let cli = Cli::parse();

    match &cli.command {
        Command::Start { config } => {
            let config: FaucetConfig =
                load_config(config).context("Failed to load configuration file")?;

            let faucet_state = FaucetState::new(config.clone()).await?;

            info!(target: COMPONENT, %config, "Initializing server");

            let app = Router::new()
                .route("/", get(get_index_html))
                .route("/index.js", get(get_index_js))
                .route("/index.css", get(get_index_css))
                .route("/get_metadata", get(get_metadata))
                .route("/get_tokens", post(get_tokens))
                .layer(
                    ServiceBuilder::new()
                        .layer(TraceLayer::new_for_http())
                        .layer(SetResponseHeaderLayer::if_not_present(
                            http::header::CACHE_CONTROL,
                            HeaderValue::from_static("no-cache"),
                        ))
                        .layer(
                            CorsLayer::new()
                                .allow_origin(tower_http::cors::Any)
                                .allow_methods(tower_http::cors::Any),
                        ),
                )
                .with_state(faucet_state);

            let socket_addr = config.endpoint.socket_addrs(|| None)?.into_iter().next().ok_or(
                anyhow::anyhow!("Couldn't get any socket addrs for endpoint: {}", config.endpoint),
            )?;
            let listener =
                TcpListener::bind(socket_addr).await.context("Failed to bind TCP listener")?;

            info!(target: COMPONENT, endpoint = %config.endpoint, "Server started");

            axum::serve(listener, app).await.unwrap();
        },

        Command::CreateFaucetAccount {
            config_path,
            output_path,
            token_symbol,
            decimals,
            max_supply,
        } => {
            println!("Generating new faucet account. This may take a few minutes...");

            let config: FaucetConfig =
                load_config(config_path).context("Failed to load configuration file")?;

            let (_, root_block_header, _) = initialize_faucet_client(&config).await?;

            let current_dir =
                std::env::current_dir().context("Failed to open current directory")?;

            let mut rng = ChaCha20Rng::from_seed(rand::random());

            let secret = SecretKey::with_rng(&mut get_rpo_random_coin(&mut rng));

            let (account, account_seed) = create_basic_fungible_faucet(
                rng.gen(),
                (&root_block_header).try_into().context("Failed to create anchor block")?,
                TokenSymbol::try_from(token_symbol.as_str())
                    .context("Failed to parse token symbol")?,
                *decimals,
                Felt::try_from(*max_supply)
                    .expect("max supply value is greater than or equal to the field modulus"),
                AccountStorageMode::Public,
                AuthScheme::RpoFalcon512 { pub_key: secret.public_key() },
            )
            .context("Failed to create basic fungible faucet account")?;

            let account_data =
                AccountData::new(account, Some(account_seed), AuthSecretKey::RpoFalcon512(secret));

            let output_path = current_dir.join(output_path);
            account_data
                .write(&output_path)
                .context("Failed to write account data to file")?;

            println!("Faucet account file successfully created at: {output_path:?}");
        },

        Command::Init { config_path, faucet_account_path } => {
            let current_dir =
                std::env::current_dir().context("Failed to open current directory")?;

            let config_file_path = current_dir.join(config_path);

            let config = FaucetConfig {
                faucet_account_path: faucet_account_path.into(),
                ..FaucetConfig::default()
            };

            let config_as_toml_string =
                toml::to_string(&config).context("Failed to serialize default config")?;

            std::fs::write(&config_file_path, config_as_toml_string)
                .context("Error writing config to file")?;

            println!("Config file successfully created at: {config_file_path:?}");
        },
    }

    Ok(())
}

/// The static website files embedded by the build.rs script.
mod static_resources {
    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}

/// Generates [`LongVersion`] using the metadata generated by build.rs.
fn long_version() -> LongVersion {
    // Use optional to allow for build script embedding failure.
    LongVersion {
        version: env!("CARGO_PKG_VERSION"),
        sha: option_env!("VERGEN_GIT_SHA").unwrap_or_default(),
        branch: option_env!("VERGEN_GIT_BRANCH").unwrap_or_default(),
        dirty: option_env!("VERGEN_GIT_DIRTY").unwrap_or_default(),
        features: option_env!("VERGEN_CARGO_FEATURES").unwrap_or_default(),
        rust_version: option_env!("VERGEN_RUSTC_SEMVER").unwrap_or_default(),
        host: option_env!("VERGEN_RUSTC_HOST_TRIPLE").unwrap_or_default(),
        target: option_env!("VERGEN_CARGO_TARGET_TRIPLE").unwrap_or_default(),
        opt_level: option_env!("VERGEN_CARGO_OPT_LEVEL").unwrap_or_default(),
        debug: option_env!("VERGEN_CARGO_DEBUG").unwrap_or_default(),
    }
}
