// FAUCET INITIALIZER
// ================================================================================================

use actix_cors::Cors;
use actix_files::Files;
use actix_web::{
    middleware::{DefaultHeaders, Logger},
    web, App, HttpServer,
};
use tracing::info;

use crate::{
    config::FaucetConfig,
    handlers::{get_metadata, get_tokens},
    utils::FaucetState,
    COMPONENT,
};

pub async fn serve(config: FaucetConfig, faucet_state: FaucetState) -> std::io::Result<()> {
    info!(target: COMPONENT, %config, "Initializing server");

    info!("Server is now running on: {}", config.as_url());

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
                Files::new("/", "crates/faucet/src/static")
                    .use_etag(false)
                    .use_last_modified(false)
                    .index_file("index.html"),
            )
    })
    .bind((config.endpoint.host, config.endpoint.port))?
    .run()
    .await?;

    Ok(())
}
