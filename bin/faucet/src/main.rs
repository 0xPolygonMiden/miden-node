mod client;
mod config;
mod errors;
mod handlers;
mod state;

use std::path::PathBuf;

use actix_cors::Cors;
use actix_files::Files;
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
            let config: FaucetConfig = load_config(config.as_path())
                .extract()
                .map_err(|err| FaucetError::ConfigurationError(err.to_string()))?;

            let faucet_state = FaucetState::new(config.clone()).await?;

            info!(target: COMPONENT, %config, "Initializing server");

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
                    .service(
                        Files::new("/", "bin/faucet/src/static")
                            .use_etag(false)
                            .use_last_modified(false)
                            .index_file("index.html"),
                    )
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

            let mut config = current_dir.clone();

            config.push(config_path);
        },
    }

    Ok(())
}
