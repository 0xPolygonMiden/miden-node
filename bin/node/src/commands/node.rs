use std::path::PathBuf;

use url::Url;

use super::{ENV_RPC_URL, ENV_STORE_DIRECTORY};

#[derive(clap::Subcommand)]
pub enum NodeCommand {
    /// Runs all three node components in the same process.
    ///
    /// The gRPC endpoints for the store and block-producer will each be assigned a random open
    /// port on localhost (127.0.0.1:0).
    Start {
        /// Url at which to serve the RPC component's gRPC API.
        #[arg(env = ENV_RPC_URL)]
        rpc_url: Url,

        /// Directory in which the Store component should store the database and raw block data.
        #[arg(env = ENV_STORE_DIRECTORY)]
        store_data_directory: PathBuf,
    },
}
