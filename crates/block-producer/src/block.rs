use std::collections::{BTreeMap, BTreeSet};

use miden_node_proto::{
    errors::{ConversionError, MissingFieldHelper},
    generated::responses::GetBlockInputsResponse,
    AccountInputRecord, NullifierWitness,
};
use miden_objects::{
    accounts::AccountId,
    crypto::{
        hash::rpo::RpoDigest,
        merkle::{MerklePath, MmrPeaks, SmtProof},
    },
    notes::{NoteId, Nullifier},
    BlockHeader, Digest,
};

use crate::store::BlockInputsError;

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

    /// List of unauthenticated notes found in the store
    pub found_unauthenticated_notes: BTreeSet<NoteId>,
}

#[derive(Clone, Debug, Default)]
pub struct AccountWitness {
    pub hash: Digest,
    pub proof: MerklePath,
}

impl TryFrom<GetBlockInputsResponse> for BlockInputs {
    type Error = BlockInputsError;

    fn try_from(response: GetBlockInputsResponse) -> Result<Self, Self::Error> {
        let block_header: BlockHeader = response
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
                response
                    .mmr_peaks
                    .into_iter()
                    .map(TryInto::try_into)
                    .collect::<Result<_, _>>()?,
            )?
        };

        let accounts = response
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
            .collect::<Result<BTreeMap<_, _>, ConversionError>>()?;

        let nullifiers = response
            .nullifiers
            .into_iter()
            .map(|entry| {
                let witness: NullifierWitness = entry.try_into()?;
                Ok((witness.nullifier, witness.proof))
            })
            .collect::<Result<BTreeMap<_, _>, ConversionError>>()?;

        let found_unauthenticated_notes = response
            .found_unauthenticated_notes
            .into_iter()
            .map(|digest| Ok(RpoDigest::try_from(digest)?.into()))
            .collect::<Result<_, ConversionError>>()?;

        Ok(Self {
            block_header,
            chain_peaks,
            accounts,
            nullifiers,
            found_unauthenticated_notes,
        })
    }
}
