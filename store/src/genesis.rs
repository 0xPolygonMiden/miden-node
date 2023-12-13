use std::time::{SystemTime, UNIX_EPOCH};

use miden_crypto::{
    dsa::rpo_falcon512,
    merkle::{EmptySubtreeRoots, MerkleError, MmrPeaks, SimpleSmt, TieredSmt},
    Felt,
};
use miden_lib::{faucets::create_basic_fungible_faucet, wallets::create_basic_wallet, AuthScheme};
use miden_node_proto::block_header;
use miden_objects::{
    accounts::Account, assets::TokenSymbol, notes::NOTE_LEAF_DEPTH, BlockHeader, Digest,
};
use serde::{Deserialize, Serialize};

use crate::state::ACCOUNT_DB_DEPTH;

/// Generates the header of the genesis block. The timestamp is currently hardcoded to be the UNIX epoch.
fn genesis_header() -> block_header::BlockHeader {
    block_header::BlockHeader {
        prev_hash: Some(Digest::default().into()),
        block_num: 0,
        chain_root: Some(MmrPeaks::new(0, Vec::new()).unwrap().hash_peaks().into()),
        account_root: Some(EmptySubtreeRoots::entry(ACCOUNT_DB_DEPTH, 0).into()),
        nullifier_root: Some(TieredSmt::default().root().into()),
        note_root: Some(EmptySubtreeRoots::entry(NOTE_LEAF_DEPTH, 0).into()),
        batch_root: Some(Digest::default().into()),
        proof_hash: Some(Digest::default().into()),
        version: 0,
        timestamp: 0,
    }
}

/// Represents the state at genesis, which will be used to derive the genesis block.
#[derive(Serialize, Deserialize)]
pub struct GenesisState {
    pub accounts: Vec<Account>,
}

impl GenesisState {
    pub fn new(pub_key: rpo_falcon512::PublicKey) -> Self {
        let mut accounts = Vec::new();

        // fungible asset faucet
        {
            let (account, _) = create_basic_fungible_faucet(
                [0; 32],
                TokenSymbol::new("TODO").unwrap(),
                9,
                Felt::from(1_000_000_000_u64),
                AuthScheme::RpoFalcon512 { pub_key },
            )
            .unwrap();

            accounts.push(account);
        }

        // basic wallet account
        {
            let (account, _) = create_basic_wallet(
                [0; 32],
                AuthScheme::RpoFalcon512 { pub_key },
                miden_objects::accounts::AccountType::RegularAccountUpdatableCode,
            )
            .unwrap();

            accounts.push(account);
        }

        Self { accounts }
    }
}

impl TryFrom<GenesisState> for block_header::BlockHeader {
    type Error = MerkleError;

    fn try_from(value: GenesisState) -> Result<Self, Self::Error> {
        let account_smt = SimpleSmt::with_leaves(
            ACCOUNT_DB_DEPTH,
            value
                .accounts
                .into_iter()
                .map(|account| (account.id().into(), account.hash().into())),
        )?;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("we are after 1970")
            .as_millis() as u64;

        let block_header = BlockHeader::new(
            Digest::default(),
            Felt::from(0_u64),
            MmrPeaks::new(0, Vec::new()).unwrap().hash_peaks().into(),
            account_smt.root(),
            TieredSmt::default().root().into(),
            *EmptySubtreeRoots::entry(NOTE_LEAF_DEPTH, 0),
            Digest::default(),
            Digest::default(),
            Felt::from(0_u64),
            // FIXME: timestamp and version goes in json
            timestamp.into(),
        );

        Ok(block_header.into())
    }
}
