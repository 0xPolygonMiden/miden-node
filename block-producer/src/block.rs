use std::collections::BTreeMap;

use miden_node_proto::{
    errors::{MissingFieldHelper, ParseError},
    generated::responses::GetBlockInputsResponse,
    AccountInputRecord, NullifierWitness,
};
use miden_objects::{
    accounts::AccountId,
    crypto::merkle::{MerklePath, MmrPeaks, SmtProof},
    notes::{NoteEnvelope, Nullifier},
    BlockHeader, Digest,
};

#[derive(Debug, Clone)]
pub struct Block {
    pub header: BlockHeader,
    pub updated_accounts: Vec<(AccountId, Digest)>,
    pub created_notes: BTreeMap<u64, NoteEnvelope>,
    pub produced_nullifiers: Vec<Nullifier>,
    // TODO:
    // - full states for updated public accounts
    // - full states for created public notes
    // - zk proof
}

impl Block {
    pub fn hash(&self) -> Digest {
        todo!()
    }
}

// BLOCK INPUTS
// ================================================================================================

/// Information needed from the store to build a block
#[derive(Clone, Debug)]
pub struct BlockInputs {
    /// Previous block header
    pub block_header: BlockHeader,

    /// MMR peaks for the current chain state
    pub chain_peaks: MmrPeaks,

    /// The hashes of the requested accounts and their authentication paths
    pub accounts: BTreeMap<AccountId, AccountWitness>,

    /// The requested nullifiers and their authentication paths
    pub nullifiers: BTreeMap<Nullifier, SmtProof>,
}

#[derive(Clone, Debug, Default)]
pub struct AccountWitness {
    pub hash: Digest,
    pub proof: MerklePath,
}

impl TryFrom<GetBlockInputsResponse> for BlockInputs {
    type Error = ParseError;

    fn try_from(get_block_inputs: GetBlockInputsResponse) -> Result<Self, Self::Error> {
        let block_header: BlockHeader = get_block_inputs
            .block_header
            .ok_or(GetBlockInputsResponse::missing_field(stringify!(block_header)))?
            .try_into()?;

        let chain_peaks = {
            // setting the number of leaves to the current block number gives us one leaf less than
            // what is currently in the chain MMR (i.e., chain MMR with block_num = 1 has 2 leave);
            // this is because GetBlockInputs returns the state of the chain MMR as of one block
            // ago so that block_header.chain_root matches the hash of MMR peaks.
            let num_leaves = block_header.block_num() as usize;

            MmrPeaks::new(
                num_leaves,
                get_block_inputs
                    .mmr_peaks
                    .into_iter()
                    .map(|peak| peak.try_into())
                    .collect::<Result<_, Self::Error>>()?,
            )
            .map_err(Self::Error::MmrPeaksError)?
        };

        let accounts = get_block_inputs
            .account_states
            .into_iter()
            .map(|entry| {
                let domain: AccountInputRecord = entry.try_into()?;
                let witness = AccountWitness {
                    hash: domain.account_hash,
                    proof: domain.proof,
                };
                Ok((domain.account_id, witness))
            })
            .collect::<Result<BTreeMap<_, _>, ParseError>>()?;

        let nullifiers = get_block_inputs
            .nullifiers
            .into_iter()
            .map(|entry| {
                let witness: NullifierWitness = entry.try_into()?;
                Ok((witness.nullifier, witness.proof))
            })
            .collect::<Result<BTreeMap<_, _>, ParseError>>()?;

        Ok(Self {
            block_header,
            chain_peaks,
            accounts,
            nullifiers,
        })
    }
}
