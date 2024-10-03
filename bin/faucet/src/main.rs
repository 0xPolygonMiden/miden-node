mod client;
mod config;
mod errors;
mod handlers;
mod state;

use std::{fs::File, io::Write, path::PathBuf};

use axum::{
    routing::{get, post},
    Router,
};
use clap::{Parser, Subcommand};
use errors::FaucetError;
use http::HeaderValue;
use miden_node_utils::{config::load_config, version::LongVersion};
use state::FaucetState;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, set_header::SetResponseHeaderLayer, trace::TraceLayer};
use tracing::info;

use crate::{
    config::FaucetConfig,
    handlers::{get_index, get_metadata, get_static_file, get_tokens},
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

    /// Generates default configuration file for the faucet
    Init {
        #[arg(short, long, default_value = FAUCET_CONFIG_FILE_PATH)]
        config_path: String,
    },
}

// MAIN
// =================================================================================================

#[tokio::main]
async fn main() -> Result<(), FaucetError> {
    miden_node_utils::logging::setup_logging()
        .map_err(|err| FaucetError::StartError(err.to_string()))?;

    let cli = Cli::parse();

    match &cli.command {
        Command::Start { config } => {
            let config: FaucetConfig = load_config(config)
                .map_err(|err| FaucetError::ConfigurationError(err.to_string()))?;

            let faucet_state = FaucetState::new(config.clone()).await?;

            info!(target: COMPONENT, %config, "Initializing server");

            let app = Router::new()
                .route("/", get(get_index))
                .route("/get_metadata", get(get_metadata))
                .route("/get_tokens", post(get_tokens))
                .route("/*path", get(get_static_file))
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

            let endpoint_url = config.endpoint_url();

            let listener = TcpListener::bind((config.endpoint.host, config.endpoint.port))
                .await
                .map_err(|err| FaucetError::StartError(err.to_string()))?;

            info!("Server is now running on: {}", endpoint_url);

            axum::serve(listener, app).await.unwrap();
        },
        Command::Init { config_path } => {
            let current_dir = std::env::current_dir().map_err(|err| {
                FaucetError::ConfigurationError(format!("failed to open current directory: {err}"))
            })?;

            let config_file_path = current_dir.join(config_path);
            let config = FaucetConfig::default();
            let config_as_toml_string = toml::to_string(&config).map_err(|err| {
                FaucetError::ConfigurationError(format!(
                    "Failed to serialize default config: {err}"
                ))
            })?;

            let mut file_handle =
                File::options().write(true).create_new(true).open(&config_file_path).map_err(
                    |err| FaucetError::ConfigurationError(format!("Error opening the file: {err}")),
                )?;

            file_handle.write(config_as_toml_string.as_bytes()).map_err(|err| {
                FaucetError::ConfigurationError(format!("Error writing to file: {err}"))
            })?;

            println!("Config file successfully created at: {:?}", config_file_path);
        },
    }

    Ok(())
}

/// The static website files embedded by the build.rs script.
mod static_resources {
    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}

/// Generates [LongVersion] using the metadata generated by build.rs.
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
