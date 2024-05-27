mod client;
mod config;
mod errors;
mod handlers;
mod utils;

use std::path::PathBuf;

use actix_cors::Cors;
use actix_files::Files;
use actix_web::{
    middleware::{DefaultHeaders, Logger},
    web, App, HttpServer,
};
use errors::FaucetError;
use miden_node_utils::config::load_config;
use tracing::info;
use utils::build_faucet_state;

use crate::{
    config::FaucetConfig,
    handlers::{get_metadata, get_tokens},
};

// CONSTANTS
// =================================================================================================

const COMPONENT: &str = "miden-faucet";

const FAUCET_CONFIG_FILE_PATH: &str = "config/miden-faucet.toml";

// MAIN
// =================================================================================================

#[actix_web::main]
async fn main() -> Result<(), FaucetError> {
    miden_node_utils::logging::setup_logging()
        .map_err(|err| FaucetError::StartError(err.to_string()))?;

    let config: FaucetConfig = load_config(PathBuf::from(FAUCET_CONFIG_FILE_PATH).as_path())
        .extract()
        .map_err(|err| FaucetError::ConfigurationError(err.to_string()))?;

    let faucet_state = build_faucet_state(config.clone()).await?;

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

    Ok(())
}
