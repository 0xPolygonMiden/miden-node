pub mod cli;
use anyhow::Result;
use clap::Parser;
use cli::{Admin, Cli, Command};
use miden_node_proto::control_plane::{api_client as control_plane_client, ShutdownRequest};
use miden_node_rpc::{config::RpcTopLevelConfig, server};
use miden_node_utils::config::load_config;

#[tokio::main]
async fn main() -> Result<()> {
    miden_node_utils::logging::setup_logging()?;

    let cli = Cli::parse();

    let config: RpcTopLevelConfig = load_config(cli.config.as_path()).extract()?;

    match cli.command {
        Command::Serve => {
            server::serve(config.rpc, config.control_plane).await?;
        },
        Command::Admin(command) => admin(config, command).await?,
    }

    Ok(())
}

/// Sends an administrative gRPC request as specified by `command`.
///
/// The request is sent to the endpoint defined in `config`.
async fn admin(
    config: RpcTopLevelConfig,
    command: Admin,
) -> Result<()> {
    let endpoint = format!(
        "http://{}:{}",
        config.control_plane.endpoint.host, config.control_plane.endpoint.port
    );
    let mut client = control_plane_client::ApiClient::connect(endpoint).await?;

    match command {
        Admin::Shutdown => {
            let request = tonic::Request::new(ShutdownRequest {});
            let response = client.shutdown(request).await?.into_inner();
            println!("{:?}", response);
            Ok(())
        },
    }
}
