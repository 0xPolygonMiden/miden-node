use std::path::PathBuf;

use url::Url;

use super::{ENV_STORE_DIRECTORY, ENV_STORE_URL};

#[derive(clap::Subcommand)]
pub enum StoreCommand {
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
