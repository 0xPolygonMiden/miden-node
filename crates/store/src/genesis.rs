use miden_objects::{
    accounts::Account,
    crypto::merkle::{EmptySubtreeRoots, MerkleError, MmrPeaks, SimpleSmt, Smt},
    notes::NOTE_LEAF_DEPTH,
    utils::serde::{ByteReader, ByteWriter, Deserializable, DeserializationError, Serializable},
    BlockHeader, Digest, ACCOUNT_TREE_DEPTH, GENESIS_BLOCK,
};

// GENESIS STATE
// ================================================================================================

/// Represents the state at genesis, which will be used to derive the genesis block.
#[derive(Debug, PartialEq, Eq)]
pub struct GenesisState {
    pub accounts: Vec<Account>,
    pub version: u64,
    pub timestamp: u32,
}

impl GenesisState {
    pub fn new(accounts: Vec<Account>, version: u64, timestamp: u32) -> Self {
        Self { accounts, version, timestamp }
    }

    /// Returns the block header and the account SMT
    pub fn into_block_parts(
        self,
    ) -> Result<(BlockHeader, SimpleSmt<ACCOUNT_TREE_DEPTH>), MerkleError> {
        let account_smt: SimpleSmt<ACCOUNT_TREE_DEPTH> = SimpleSmt::with_leaves(
            self.accounts
                .into_iter()
                .map(|account| (account.id().into(), account.hash().into())),
        )?;

        let block_header = BlockHeader::new(
            Digest::default(),
            GENESIS_BLOCK,
            MmrPeaks::new(0, Vec::new()).unwrap().hash_peaks(),
            account_smt.root(),
            Smt::default().root(),
            *EmptySubtreeRoots::entry(NOTE_LEAF_DEPTH, 0),
            Digest::default(),
            Digest::default(),
            self.version
                .try_into()
                .expect("version value is greater than or equal to the field modulus"),
            self.timestamp,
        );

        Ok((block_header, account_smt))
    }
}

// SERIALIZATION
// ================================================================================================

impl Serializable for GenesisState {
    fn write_into<W: ByteWriter>(&self, target: &mut W) {
        assert!(self.accounts.len() <= u64::MAX as usize, "too many accounts in GenesisState");
        target.write_usize(self.accounts.len());
        target.write_many(&self.accounts);

        target.write_u64(self.version);
        target.write_u32(self.timestamp);
    }
}

impl Deserializable for GenesisState {
    fn read_from<R: ByteReader>(source: &mut R) -> Result<Self, DeserializationError> {
        let num_accounts = source.read_usize()?;
        let accounts = source.read_many::<Account>(num_accounts)?;

        let version = source.read_u64()?;
        let timestamp = source.read_u32()?;

        Ok(Self::new(accounts, version, timestamp))
    }
}
