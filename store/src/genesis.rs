use miden_crypto::{
    merkle::{EmptySubtreeRoots, MerkleError, MmrPeaks, SimpleSmt, TieredSmt},
    Word,
};
use miden_objects::{
    accounts::Account,
    notes::NOTE_LEAF_DEPTH,
    utils::serde::{ByteReader, ByteWriter, Deserializable, DeserializationError, Serializable},
    BlockHeader, Digest,
};

use crate::state::ACCOUNT_DB_DEPTH;

pub const GENESIS_BLOCK_NUM: u32 = 0;

#[derive(Debug)]
pub struct AccountAndSeed {
    pub account: Account,
    pub seed: Word,
}

impl Serializable for AccountAndSeed {
    fn write_into<W: ByteWriter>(
        &self,
        target: &mut W,
    ) {
        target.write(self.account.to_bytes());
        target.write(self.seed.to_bytes());
    }
}

impl Deserializable for AccountAndSeed {
    fn read_from<R: ByteReader>(source: &mut R) -> Result<Self, DeserializationError> {
        let account = Account::read_from(source)?;
        let seed = Word::read_from(source)?;
        Ok(AccountAndSeed { account, seed })
    }
}

/// Represents the state at genesis, which will be used to derive the genesis block.
pub struct GenesisState {
    pub accounts: Vec<AccountAndSeed>,
    pub version: u64,
    pub timestamp: u64,
}

impl GenesisState {
    pub fn new(
        accounts: Vec<AccountAndSeed>,
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
            self.accounts.into_iter().map(|account_and_seed| {
                (account_and_seed.account.id().into(), account_and_seed.account.hash().into())
            }),
        )?;

        let block_header = BlockHeader::new(
            Digest::default(),
            GENESIS_BLOCK_NUM,
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

// SERIALIZATION
// ================================================================================================

impl Serializable for GenesisState {
    fn write_into<W: ByteWriter>(
        &self,
        target: &mut W,
    ) {
        assert!(self.accounts.len() <= u64::MAX as usize, "too many accounts in GenesisState");
        target.write_u64(self.accounts.len() as u64);

        for account in self.accounts.iter() {
            account.write_into(target);
        }

        target.write_u64(self.version);
        target.write_u64(self.timestamp);
    }
}

impl Deserializable for GenesisState {
    fn read_from<R: ByteReader>(source: &mut R) -> Result<Self, DeserializationError> {
        let num_accounts = source.read_u64()? as usize;
        let accounts = AccountAndSeed::read_batch_from(source, num_accounts)?;

        let version = source.read_u64()?;
        let timestamp = source.read_u64()?;

        Ok(Self::new(accounts, version, timestamp))
    }
}
