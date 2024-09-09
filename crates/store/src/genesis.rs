use miden_objects::{
    accounts::{delta::AccountUpdateDetails, Account},
    block::{Block, BlockAccountUpdate},
    crypto::merkle::{EmptySubtreeRoots, MmrPeaks, SimpleSmt, Smt},
    notes::NOTE_LEAF_DEPTH,
    utils::serde::{ByteReader, ByteWriter, Deserializable, DeserializationError, Serializable},
    BlockHeader, Digest, ACCOUNT_TREE_DEPTH, GENESIS_BLOCK,
};

use crate::errors::GenesisError;

// GENESIS STATE
// ================================================================================================

/// Represents the state at genesis, which will be used to derive the genesis block.
#[derive(Debug, PartialEq, Eq)]
pub struct GenesisState {
    pub accounts: Vec<Account>,
    pub version: u32,
    pub timestamp: u32,
}

impl GenesisState {
    pub fn new(accounts: Vec<Account>, version: u32, timestamp: u32) -> Self {
        Self { accounts, version, timestamp }
    }

    /// Returns the block header and the account SMT
    pub fn into_block(self) -> Result<Block, GenesisError> {
        let accounts: Vec<BlockAccountUpdate> = self
            .accounts
            .iter()
            .map(|account| {
                let account_update_details = if account.id().is_public() {
                    AccountUpdateDetails::New(account.clone())
                } else {
                    AccountUpdateDetails::Private
                };

                BlockAccountUpdate::new(
                    account.id(),
                    account.hash(),
                    account_update_details,
                    vec![],
                )
            })
            .collect();

        let account_smt: SimpleSmt<ACCOUNT_TREE_DEPTH> = SimpleSmt::with_leaves(
            accounts
                .iter()
                .map(|update| (update.account_id().into(), update.new_state_hash().into())),
        )?;

        let header = BlockHeader::new(
            self.version,
            Digest::default(),
            GENESIS_BLOCK,
            MmrPeaks::new(0, Vec::new()).unwrap().hash_peaks(),
            account_smt.root(),
            Smt::default().root(),
            *EmptySubtreeRoots::entry(NOTE_LEAF_DEPTH, 0),
            Digest::default(),
            Digest::default(),
            self.timestamp,
        );

        Block::new(header, accounts, vec![], vec![]).map_err(Into::into)
    }
}

// SERIALIZATION
// ================================================================================================

impl Serializable for GenesisState {
    fn write_into<W: ByteWriter>(&self, target: &mut W) {
        assert!(self.accounts.len() <= u64::MAX as usize, "too many accounts in GenesisState");
        target.write_usize(self.accounts.len());
        target.write_many(&self.accounts);

        target.write_u32(self.version);
        target.write_u32(self.timestamp);
    }
}

impl Deserializable for GenesisState {
    fn read_from<R: ByteReader>(source: &mut R) -> Result<Self, DeserializationError> {
        let num_accounts = source.read_usize()?;
        let accounts = source.read_many::<Account>(num_accounts)?;

        let version = source.read_u32()?;
        let timestamp = source.read_u32()?;

        Ok(Self::new(accounts, version, timestamp))
    }
}
