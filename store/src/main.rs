mod cli;
use anyhow::{anyhow, Result};
use clap::Parser;
use cli::{Cli, Command, Query};
use hex::ToHex;
use miden_crypto::merkle::{path_to_text, TieredSmtProof};
use miden_node_proto::generated::{
    account::AccountId,
    requests::{
        CheckNullifiersRequest, GetBlockHeaderByNumberRequest, GetBlockInputsRequest,
        GetTransactionInputsRequest, ListAccountsRequest, ListNotesRequest, ListNullifiersRequest,
        SyncStateRequest,
    },
    smt::SmtOpening,
    store::api_client,
};
use miden_node_store::{config::StoreTopLevelConfig, db::Db, server};
use miden_node_utils::config::load_config;
use miden_objects::BlockHeader;

#[tokio::main]
async fn main() -> Result<()> {
    miden_node_utils::logging::setup_logging()?;

    let cli = Cli::parse();
    let config: StoreTopLevelConfig = load_config(cli.config.as_path()).extract()?;
    let db = Db::setup(config.store.clone()).await?;

    match cli.command {
        Command::Serve { .. } => {
            server::serve(config.store, db).await?;
        },
        Command::Query(command) => query(config, command).await?,
    }

    Ok(())
}

/// Sends a gRPC request as specified by `command`.
///
/// The request is sent to the endpoint defined in `config`.
async fn query(
    config: StoreTopLevelConfig,
    command: Query,
) -> Result<()> {
    let mut client = api_client::ApiClient::connect(config.store.endpoint.to_string()).await?;

    match command {
        Query::GetBlockHeaderByNumber(args) => {
            let request = tonic::Request::new(GetBlockHeaderByNumberRequest {
                block_num: args.block_num,
            });
            let response = client.get_block_header_by_number(request).await?.into_inner();
            match response.block_header {
                Some(block_header) => {
                    let block_header: BlockHeader = block_header.try_into()?;
                    println!("{block_header:?}");
                },
                None => match args.block_num {
                    Some(block_num) => {
                        return Err(anyhow!("No block with block_num {:?} found", block_num))
                    },
                    None => {
                        return Err(anyhow!("Error, store returned no result for latest block"))
                    },
                },
            };
            Ok(())
        },
        Query::CheckNullifiers(args) => {
            let request = tonic::Request::new(CheckNullifiersRequest {
                nullifiers: args.nullifiers.clone(),
            });
            let response = client.check_nullifiers(request).await?.into_inner();
            let proofs =
                response.proofs.into_iter().map(<SmtOpening as TryInto<SmtProof>>::try_into);
            for (result, nullifier) in proofs.zip(args.nullifiers.iter()) {
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
            Ok(())
        },
        Query::SyncState(args) => {
            let request = tonic::Request::new(SyncStateRequest {
                block_num: args.block_num,
                account_ids: args.account_ids.iter().map(|&id| AccountId { id }).collect(),
                note_tags: args.note_tags.clone(),
                nullifiers: args.nullifiers.clone(),
            });
            let response = client.sync_state(request).await?.into_inner();
            println!("{:?}", response);
            Ok(())
        },
        Query::GetBlockInputs(args) => {
            let request = tonic::Request::new(GetBlockInputsRequest {
                account_ids: args.account_ids.iter().map(|&id| AccountId { id }).collect(),
                nullifiers: args.nullifiers.clone(),
            });
            let response = client.get_block_inputs(request).await?.into_inner();
            println!("{:?}", response);
            Ok(())
        },
        Query::GetTransactionInputs(args) => {
            let request = tonic::Request::new(GetTransactionInputsRequest {
                account_id: Some(AccountId {
                    id: args.account_id,
                }),
                nullifiers: args.nullifiers.clone(),
            });
            let response = client.get_transaction_inputs(request).await?.into_inner();
            println!("{:?}", response);
            Ok(())
        },
        Query::ListNullifiers => {
            let request = tonic::Request::new(ListNullifiersRequest {});
            let response = client.list_nullifiers(request).await?.into_inner();
            println!("{:?}", response);
            Ok(())
        },
        Query::ListNotes => {
            let request = tonic::Request::new(ListNotesRequest {});
            let response = client.list_notes(request).await?.into_inner();
            println!("{:?}", response);
            Ok(())
        },
        Query::ListAccounts => {
            let request = tonic::Request::new(ListAccountsRequest {});
            let response = client.list_accounts(request).await?.into_inner();
            println!("{:?}", response);
            Ok(())
        },
    }
}
