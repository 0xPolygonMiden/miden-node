use std::sync::Arc;

use async_trait::async_trait;
use miden_objects::transaction::ProvenTransaction;
use tokio::sync::Mutex;

use crate::{
    rpc::{Rpc, RpcClient},
    SharedProvenTx,
};

// TODO: Put in right module
pub enum VerifyTxError {}

// TYPE ALIASES
// ================================================================================================

type SharedMutVec<T> = Arc<Mutex<Vec<T>>>;
type ReadyQueue = SharedMutVec<SharedProvenTx>;

// READ TX SERVER
// ================================================================================================

pub struct ReadTxRpc {
    verify_tx_client: RpcClient<SharedProvenTx, Result<(), VerifyTxError>>,
}

#[async_trait]
impl Rpc<ProvenTransaction, ()> for ReadTxRpc {
    async fn handle_request(
        &self,
        proven_tx: ProvenTransaction,
    ) -> () {
        todo!()
    }
}
