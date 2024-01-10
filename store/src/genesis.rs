use miden_crypto::merkle::{EmptySubtreeRoots, MerkleError, MmrPeaks, SimpleSmt, TieredSmt};
use miden_objects::{
    accounts::{Account, AccountType},
    notes::NOTE_LEAF_DEPTH,
    utils::serde::{ByteReader, ByteWriter, Deserializable, DeserializationError, Serializable},
    BlockHeader, Digest,
};
use serde::Deserialize;

use crate::state::ACCOUNT_DB_DEPTH;

pub const GENESIS_BLOCK_NUM: u32 = 0;

#[derive(Debug, Deserialize)]
pub struct GenesisInput {
    pub version: u64,
    pub timestamp: u64,
    pub accounts: Vec<AccountInput>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum AccountInput {
    BasicWallet(BasicWalletInputs),
    BasicFungibleFaucet(BasicFungibleFaucetInputs),
}

#[derive(Debug, Deserialize)]
pub enum AuthSchemeInput {
    RpoFalcon512,
}

#[derive(Debug, Deserialize)]
pub struct BasicWalletInputs {
    pub mode: AccountType,
    pub account_seed: String,
    pub auth_seed: String,
    pub auth_scheme: AuthSchemeInput,
}

#[derive(Debug, Deserialize)]
pub struct BasicFungibleFaucetInputs {
    pub mode: AccountType,
    pub account_seed: String,
    pub auth_seed: String,
    pub auth_scheme: AuthSchemeInput,
    pub token_symbol: String,
    pub decimals: u8,
    pub max_supply: u64,
}

#[derive(Debug, Deserialize)]
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
        let accounts = Account::read_batch_from(source, num_accounts)?;

        let version = source.read_u64()?;
        let timestamp = source.read_u64()?;

        Ok(Self::new(accounts, version, timestamp))
    }
}
