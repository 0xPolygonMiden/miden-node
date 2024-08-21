mod client;
mod config;
mod errors;
mod handlers;
mod state;

use std::{fs::File, io::Write, path::PathBuf, sync::Arc, time::Duration};

use actix_cors::Cors;
use actix_web::{
    middleware::{DefaultHeaders, Logger},
    web, App, HttpServer,
};
use clap::{Parser, Subcommand};
use errors::FaucetError;
use miden_node_utils::config::load_config;
use state::FaucetState;
use tracing::info;

use crate::{
    config::FaucetConfig,
    handlers::{get_metadata, get_tokens},
};

// CONSTANTS
// =================================================================================================

const COMPONENT: &str = "miden-faucet";
const FAUCET_CONFIG_FILE_PATH: &str = "miden-faucet.toml";
const CHAIN_TIP_UPDATER_INTERVAL: u64 = 5;

// COMMANDS
// ================================================================================================

#[derive(Parser)]
#[command(version, about, long_about = None)]
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

#[actix_web::main]
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

            let client_state_clone = Arc::clone(&faucet_state.client);

            info!("Initializing chain tip updater");
            actix_web::rt::spawn(async move {
                let mut interval =
                    actix_web::rt::time::interval(Duration::from_secs(CHAIN_TIP_UPDATER_INTERVAL));
                loop {
                    let state_clone_inner = Arc::clone(&client_state_clone);
                    let mut state = state_clone_inner.lock().await;
                    state.update_current_block_number().await.unwrap();

                    interval.tick().await;
                }
            });

            info!("Server is now running on: {}", config.endpoint_url());

            HttpServer::new(move || {
                let cors = Cors::default().allow_any_origin().allow_any_method();
                App::new()
                    .app_data(web::Data::new(faucet_state.clone()))
                    .wrap(cors)
                    .wrap(Logger::default())
                    .wrap(DefaultHeaders::new().add(("Cache-Control", "no-cache")))
                    .service(get_metadata)
                    .service(get_tokens)
                    .service(actix_web_static_files::ResourceFiles::new(
                        "/",
                        static_resources::generate(),
                    ))
            })
            .bind((config.endpoint.host, config.endpoint.port))
            .map_err(|err| FaucetError::StartError(err.to_string()))?
            .run()
            .await
            .map_err(|err| FaucetError::StartError(err.to_string()))?;
        },
        Command::Init { config_path } => {
            let current_dir = std::env::current_dir().map_err(|err| {
                FaucetError::ConfigurationError(format!("failed to open current directory: {err}"))
            })?;

            let mut config_file_path = current_dir.clone();
            config_file_path.push(config_path);

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
