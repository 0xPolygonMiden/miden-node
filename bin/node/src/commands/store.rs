use std::path::PathBuf;

use anyhow::Context;
use miden_node_store::server::Store;
use miden_node_utils::grpc::UrlExt;
use url::Url;

use super::{genesis::GenesisInput, ENV_STORE_DIRECTORY, ENV_STORE_URL};

#[derive(clap::Subcommand)]
pub enum StoreCommand {
    /// Dumps the default genesis configuration to stdout.
    ///
    /// Use this as a starting point to configure the genesis data created by `make-genesis`.
    DumpGenesis,
    Init,
    /// Starts the store component.
    Start {
        /// Url at which to serve the gRPC API.
        #[arg(env = ENV_STORE_URL)]
        url: Url,

        /// Directory in which to store the database and raw block data.
        #[arg(env = ENV_STORE_DIRECTORY)]
        data_directory: PathBuf,
    },
}

#[derive(clap::Args)]
pub struct StoreConfig {}

impl StoreCommand {
    /// Executes the subcommand as described by each variants documentation.
    pub async fn handle(self) -> anyhow::Result<()> {
        match self {
            StoreCommand::DumpGenesis => Ok({
                let to_dump = toml::to_string(&GenesisInput::default())
                    .expect("Default genesis.toml should serialize");

                println!("{to_dump}");
            }),
            StoreCommand::Init => todo!(),
            StoreCommand::Start { url, data_directory } => {
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
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ensures that [GenesisInput::default()] is serializable since otherwise we panic.
    #[tokio::test]
    async fn dump_config_succeeds() {
        StoreCommand::DumpGenesis.handle().await.unwrap();
    }
}
