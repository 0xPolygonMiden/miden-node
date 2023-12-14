use miden_crypto::{
    merkle::{EmptySubtreeRoots, MerkleError, MmrPeaks, SimpleSmt, TieredSmt},
    Felt,
};
use miden_objects::{accounts::Account, notes::NOTE_LEAF_DEPTH, BlockHeader, Digest};
use serde::{Deserialize, Serialize};

// FIXME: This is a duplicate of the constant in `store::state`
pub(crate) const ACCOUNT_DB_DEPTH: u8 = 64;

/// Represents the state at genesis, which will be used to derive the genesis block.
#[derive(Serialize, Deserialize)]
pub struct GenesisState {
    pub accounts: Vec<Account>,
    pub version: u64,
    pub timestamp: u64,
}

impl GenesisState {
    pub fn new(
        accounts: Vec<Account>,
        version: u64,
        timestamp: u64,
    ) -> Self {
        Self {
            accounts,
            version,
            timestamp,
        }
    }

    /// Returns the block header and the account SMT
    pub fn into_block_parts(self) -> Result<(BlockHeader, SimpleSmt), MerkleError> {
        let account_smt = SimpleSmt::with_leaves(
            ACCOUNT_DB_DEPTH,
            self.accounts
                .into_iter()
                .map(|account| (account.id().into(), account.hash().into())),
        )?;

        let block_header = BlockHeader::new(
            Digest::default(),
            Felt::from(0_u64),
            MmrPeaks::new(0, Vec::new()).unwrap().hash_peaks(),
            account_smt.root(),
            TieredSmt::default().root(),
            *EmptySubtreeRoots::entry(NOTE_LEAF_DEPTH, 0),
            Digest::default(),
            Digest::default(),
            self.version.into(),
            self.timestamp.into(),
        );

        Ok((block_header, account_smt))
    }
}
