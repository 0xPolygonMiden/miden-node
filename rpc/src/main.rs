pub mod cli;
use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command, Request};
use hex::ToHex;
use miden_crypto::merkle::{path_to_text, TieredSmtProof};
use miden_node_proto::{requests::CheckNullifiersRequest, rpc::api_client, tsmt::NullifierProof};
use miden_node_rpc::{config::RpcConfig, server::api};
use miden_node_utils::Config;

#[tokio::main]
async fn main() -> Result<()> {
    miden_node_utils::logging::setup_logging()?;

    let cli = Cli::parse();

    let config = RpcConfig::load_config(cli.config.as_deref()).extract()?;

    match cli.command {
        Command::Serve => {
            api::serve(config).await?;
        },
        Command::Request(req) => match req {
            Request::CheckNullifiers { nullifiers } => {
                let host_port = format!("http://{}:{}", config.endpoint.host, config.endpoint.port);
                let mut client = api_client::ApiClient::connect(host_port).await?;
                let request = tonic::Request::new(CheckNullifiersRequest {
                    nullifiers: nullifiers.clone(),
                });
                let response = client.check_nullifiers(request).await?.into_inner();
                let proofs = response
                    .proofs
                    .into_iter()
                    .map(<NullifierProof as TryInto<TieredSmtProof>>::try_into);

                for (result, nullifier) in proofs.zip(nullifiers.iter()) {
                    match result {
                        Ok(proof) => {
                            let (path, leaf) = proof.into_parts();
                            println!(
                                "{} merkle_path: {:?} leaf: {:?}",
                                nullifier.encode_hex::<String>(),
                                path_to_text(&path).expect("Formatting merkle path failed"),
                                leaf,
                            )
                        },
                        Err(e) => println!("{} {:?}", nullifier.encode_hex::<String>(), e),
                    }
                }
            },
        },
    }

    Ok(())
}
