use std::collections::BTreeMap;

use super::*;
use crate::store::TxInputsError;

// MOCK STORE
// =================================================================================================

struct MockStore {
    /// Map account id -> account hash
    accounts: Arc<RwLock<BTreeMap<AccountId, Digest>>>,

    /// Stores the nullifiers of the notes that were consumed
    consumed_nullifiers: Arc<RwLock<BTreeSet<Digest>>>,
}

#[async_trait]
impl ApplyBlock for MockStore {
    async fn apply_block(
        &self,
        block: Arc<Block>,
    ) -> Result<(), ApplyBlockError> {
        // Intentionally, we take and hold both locks, to prevent calls to `get_tx_inputs()` from going through while we're updating the store's data structure
        let mut locked_accounts = self.accounts.write().await;
        let mut locked_consumed_nullifiers = self.consumed_nullifiers.write().await;

        for &(account_id, account_hash) in block.updated_accounts.iter() {
            locked_accounts.insert(account_id, account_hash);
        }

        let mut new_nullifiers: BTreeSet<Digest> = block.new_nullifiers.iter().cloned().collect();
        locked_consumed_nullifiers.append(&mut new_nullifiers);

        Ok(())
    }
}

#[async_trait]
impl Store for MockStore {
    async fn get_tx_inputs(
        &self,
        proven_tx: SharedProvenTx,
    ) -> Result<TxInputs, TxInputsError> {
        let locked_accounts = self.accounts.read().await;
        let locked_consumed_nullifiers = self.consumed_nullifiers.read().await;

        let account_hash = locked_accounts.get(&proven_tx.account_id()).cloned();

        let nullifiers = proven_tx
            .consumed_notes()
            .iter()
            .map(|note| (note.nullifier(), locked_consumed_nullifiers.contains(&note.nullifier())))
            .collect();

        Ok(TxInputs {
            account_hash,
            nullifiers,
        })
    }
}
