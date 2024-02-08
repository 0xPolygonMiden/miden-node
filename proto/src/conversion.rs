use miden_crypto::{
    merkle::{LeafIndex, MerklePath, MmrDelta, MmrPeaks, SmtLeaf, SmtProof},
    Felt, StarkField, Word,
};
use miden_objects::{
    accounts::AccountId,
    notes::{NoteEnvelope, NoteId, Nullifier},
    BlockHeader, Digest as RpoDigest,
};

use crate::{
    account, block_header,
    digest::{self, Digest},
    domain::{AccountInputRecord, BlockInputs, NullifierInputRecord},
    errors, merkle, mmr, note, requests, responses, smt,
};

impl From<[u64; 4]> for digest::Digest {
    fn from(value: [u64; 4]) -> Self {
        Self {
            d0: value[0],
            d1: value[1],
            d2: value[2],
            d3: value[3],
        }
    }
}

impl From<&[u64; 4]> for digest::Digest {
    fn from(value: &[u64; 4]) -> Self {
        (*value).into()
    }
}

impl From<[Felt; 4]> for digest::Digest {
    fn from(value: [Felt; 4]) -> Self {
        Self {
            d0: value[0].as_int(),
            d1: value[1].as_int(),
            d2: value[2].as_int(),
            d3: value[3].as_int(),
        }
    }
}

impl From<&[Felt; 4]> for digest::Digest {
    fn from(value: &[Felt; 4]) -> Self {
        (*value).into()
    }
}

impl From<RpoDigest> for digest::Digest {
    fn from(value: RpoDigest) -> Self {
        Self {
            d0: value[0].as_int(),
            d1: value[1].as_int(),
            d2: value[2].as_int(),
            d3: value[3].as_int(),
        }
    }
}

impl From<&RpoDigest> for digest::Digest {
    fn from(value: &RpoDigest) -> Self {
        (*value).into()
    }
}

impl From<digest::Digest> for [u64; 4] {
    fn from(value: digest::Digest) -> Self {
        [value.d0, value.d1, value.d2, value.d3]
    }
}

impl TryFrom<smt::SmtLeaf> for SmtLeaf {
    type Error = errors::ParseError;

    fn try_from(value: smt::SmtLeaf) -> Result<Self, Self::Error> {
        let leaf = value.leaf.ok_or(errors::ParseError::ProtobufMissingData)?;

        match leaf {
            smt::smt_leaf::Leaf::Empty(leaf_index) => {
                Ok(Self::new_empty(LeafIndex::new_max_depth(leaf_index)))
            },
            smt::smt_leaf::Leaf::Single(entry) => {
                let (key, value): (RpoDigest, Word) = entry.try_into()?;

                Ok(SmtLeaf::new_single(key, value))
            },
            smt::smt_leaf::Leaf::Multiple(entries) => {
                let domain_entries: Vec<(RpoDigest, Word)> = try_convert(entries.entries)?;

                Ok(SmtLeaf::new_multiple(domain_entries)?)
            },
        }
    }
}

impl From<SmtLeaf> for smt::SmtLeaf {
    fn from(smt_leaf: SmtLeaf) -> Self {
        use smt::smt_leaf::Leaf;

        let leaf = match smt_leaf {
            SmtLeaf::Empty(leaf_index) => Leaf::Empty(leaf_index.value()),
            SmtLeaf::Single(entry) => Leaf::Single(entry.into()),
            SmtLeaf::Multiple(entries) => Leaf::Multiple(smt::SmtLeafEntries {
                entries: convert(entries),
            }),
        };

        Self { leaf: Some(leaf) }
    }
}

impl TryFrom<smt::SmtLeafEntry> for (RpoDigest, Word) {
    type Error = errors::ParseError;

    fn try_from(entry: smt::SmtLeafEntry) -> Result<Self, Self::Error> {
        let key: RpoDigest =
            entry.key.ok_or(errors::ParseError::ProtobufMissingData)?.try_into()?;
        let value: Word = entry.value.ok_or(errors::ParseError::ProtobufMissingData)?.try_into()?;

        Ok((key, value))
    }
}

impl From<(RpoDigest, Word)> for smt::SmtLeafEntry {
    fn from((key, value): (RpoDigest, Word)) -> Self {
        Self {
            key: Some(key.into()),
            value: Some(value.into()),
        }
    }
}

impl TryFrom<smt::SmtOpening> for SmtProof {
    type Error = errors::ParseError;

    fn try_from(opening: smt::SmtOpening) -> Result<Self, Self::Error> {
        let path: MerklePath =
            opening.path.ok_or(errors::ParseError::ProtobufMissingData)?.try_into()?;
        let leaf: SmtLeaf =
            opening.leaf.ok_or(errors::ParseError::ProtobufMissingData)?.try_into()?;

        Ok(SmtProof::new(path, leaf)?)
    }
}

impl From<SmtProof> for smt::SmtOpening {
    fn from(proof: SmtProof) -> Self {
        let (path, leaf) = proof.into_parts();
        Self {
            path: Some(path.into()),
            leaf: Some(leaf.into()),
        }
    }
}

impl TryFrom<digest::Digest> for [Felt; 4] {
    type Error = errors::ParseError;

    fn try_from(value: digest::Digest) -> Result<Self, Self::Error> {
        if ![value.d0, value.d1, value.d2, value.d3]
            .iter()
            .all(|v| *v < <Felt as StarkField>::MODULUS)
        {
            Err(errors::ParseError::NotAValidFelt)
        } else {
            Ok([
                Felt::new(value.d0),
                Felt::new(value.d1),
                Felt::new(value.d2),
                Felt::new(value.d3),
            ])
        }
    }
}

impl TryFrom<digest::Digest> for RpoDigest {
    type Error = errors::ParseError;

    fn try_from(value: digest::Digest) -> Result<Self, Self::Error> {
        Ok(Self::new(value.try_into()?))
    }
}

impl TryFrom<&digest::Digest> for [Felt; 4] {
    type Error = errors::ParseError;

    fn try_from(value: &digest::Digest) -> Result<Self, Self::Error> {
        value.clone().try_into()
    }
}

impl TryFrom<&digest::Digest> for RpoDigest {
    type Error = errors::ParseError;

    fn try_from(value: &digest::Digest) -> Result<Self, Self::Error> {
        value.clone().try_into()
    }
}

impl TryFrom<block_header::BlockHeader> for BlockHeader {
    type Error = errors::ParseError;

    fn try_from(value: block_header::BlockHeader) -> Result<Self, Self::Error> {
        Ok(BlockHeader::new(
            value.prev_hash.ok_or(errors::ParseError::ProtobufMissingData)?.try_into()?,
            value.block_num,
            value.chain_root.ok_or(errors::ParseError::ProtobufMissingData)?.try_into()?,
            value.account_root.ok_or(errors::ParseError::ProtobufMissingData)?.try_into()?,
            value
                .nullifier_root
                .ok_or(errors::ParseError::ProtobufMissingData)?
                .try_into()?,
            value.note_root.ok_or(errors::ParseError::ProtobufMissingData)?.try_into()?,
            value.batch_root.ok_or(errors::ParseError::ProtobufMissingData)?.try_into()?,
            value.proof_hash.ok_or(errors::ParseError::ProtobufMissingData)?.try_into()?,
            value.version.into(),
            value.timestamp.into(),
        ))
    }
}

impl TryFrom<&block_header::BlockHeader> for BlockHeader {
    type Error = errors::ParseError;

    fn try_from(value: &block_header::BlockHeader) -> Result<Self, Self::Error> {
        value.clone().try_into()
    }
}

impl From<BlockHeader> for block_header::BlockHeader {
    fn from(header: BlockHeader) -> Self {
        Self {
            prev_hash: Some(header.prev_hash().into()),
            block_num: u64::from(header.block_num())
                .try_into()
                .expect("TODO: BlockHeader.block_num should be u64"),
            chain_root: Some(header.chain_root().into()),
            account_root: Some(header.account_root().into()),
            nullifier_root: Some(header.nullifier_root().into()),
            note_root: Some(header.note_root().into()),
            batch_root: Some(header.batch_root().into()),
            proof_hash: Some(header.proof_hash().into()),
            version: u64::from(header.version())
                .try_into()
                .expect("TODO: BlockHeader.version should be u64"),
            timestamp: header.timestamp().into(),
        }
    }
}

impl TryFrom<mmr::MmrDelta> for MmrDelta {
    type Error = errors::ParseError;

    fn try_from(value: mmr::MmrDelta) -> Result<Self, Self::Error> {
        let data: Result<Vec<RpoDigest>, errors::ParseError> =
            value.data.into_iter().map(|v| v.try_into()).collect();

        Ok(MmrDelta {
            forest: value.forest as usize,
            data: data?,
        })
    }
}

impl From<MmrDelta> for mmr::MmrDelta {
    fn from(value: MmrDelta) -> Self {
        let data: Vec<digest::Digest> = value.data.into_iter().map(|v| v.into()).collect();

        mmr::MmrDelta {
            forest: value.forest as u64,
            data,
        }
    }
}

impl From<MerklePath> for merkle::MerklePath {
    fn from(value: MerklePath) -> Self {
        let siblings: Vec<digest::Digest> = value.nodes().iter().map(|v| (*v).into()).collect();
        merkle::MerklePath { siblings }
    }
}

impl TryFrom<merkle::MerklePath> for MerklePath {
    type Error = errors::ParseError;

    fn try_from(merkle_path: merkle::MerklePath) -> Result<Self, Self::Error> {
        merkle_path.siblings.into_iter().map(|v| v.try_into()).collect()
    }
}

impl From<note::Note> for note::NoteSyncRecord {
    fn from(value: note::Note) -> Self {
        Self {
            note_index: value.note_index,
            note_hash: value.note_hash,
            sender: value.sender,
            tag: value.tag,
            merkle_path: value.merkle_path,
        }
    }
}

impl From<account::AccountId> for u64 {
    fn from(value: account::AccountId) -> Self {
        value.id
    }
}

impl From<u64> for account::AccountId {
    fn from(value: u64) -> Self {
        account::AccountId { id: value }
    }
}

impl From<AccountId> for account::AccountId {
    fn from(account_id: AccountId) -> Self {
        Self {
            id: account_id.into(),
        }
    }
}

impl TryFrom<account::AccountId> for AccountId {
    type Error = errors::ParseError;

    fn try_from(account_id: account::AccountId) -> Result<Self, Self::Error> {
        account_id.id.try_into().map_err(|_| errors::ParseError::NotAValidFelt)
    }
}

impl TryFrom<responses::AccountBlockInputRecord> for AccountInputRecord {
    type Error = errors::ParseError;

    fn try_from(
        account_input_record: responses::AccountBlockInputRecord
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            account_id: account_input_record
                .account_id
                .ok_or(errors::ParseError::ProtobufMissingData)?
                .try_into()?,
            account_hash: account_input_record
                .account_hash
                .ok_or(errors::ParseError::ProtobufMissingData)?
                .try_into()?,
            proof: account_input_record
                .proof
                .ok_or(errors::ParseError::ProtobufMissingData)?
                .try_into()?,
        })
    }
}

impl TryFrom<responses::NullifierBlockInputRecord> for NullifierInputRecord {
    type Error = errors::ParseError;

    fn try_from(
        nullifier_input_record: responses::NullifierBlockInputRecord
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            nullifier: nullifier_input_record
                .nullifier
                .ok_or(errors::ParseError::ProtobufMissingData)?
                .try_into()?,
            proof: nullifier_input_record
                .opening
                .ok_or(errors::ParseError::ProtobufMissingData)?
                .try_into()?,
        })
    }
}

impl From<NullifierInputRecord> for responses::NullifierBlockInputRecord {
    fn from(value: NullifierInputRecord) -> Self {
        Self {
            nullifier: Some(value.nullifier.into()),
            opening: Some(value.proof.into()),
        }
    }
}

impl TryFrom<responses::GetBlockInputsResponse> for BlockInputs {
    type Error = errors::ParseError;

    fn try_from(get_block_inputs: responses::GetBlockInputsResponse) -> Result<Self, Self::Error> {
        let block_header: BlockHeader = get_block_inputs
            .block_header
            .ok_or(errors::ParseError::ProtobufMissingData)?
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

        Ok(Self {
            block_header,
            chain_peaks,
            account_states: try_convert(get_block_inputs.account_states)?,
            nullifiers: try_convert(get_block_inputs.nullifiers)?,
        })
    }
}

impl From<(AccountId, RpoDigest)> for requests::AccountUpdate {
    fn from((account_id, account_hash): (AccountId, RpoDigest)) -> Self {
        Self {
            account_id: Some(account_id.into()),
            account_hash: Some(account_hash.into()),
        }
    }
}

impl From<(u64, NoteEnvelope)> for note::NoteCreated {
    fn from((note_idx, note): (u64, NoteEnvelope)) -> Self {
        Self {
            note_hash: Some(note.note_id().into()),
            sender: note.metadata().sender().into(),
            tag: note.metadata().tag().into(),
            note_index: note_idx as u32,
        }
    }
}

impl From<&Nullifier> for Digest {
    fn from(value: &Nullifier) -> Self {
        (*value).inner().into()
    }
}

impl From<Nullifier> for Digest {
    fn from(value: Nullifier) -> Self {
        value.inner().into()
    }
}

impl From<&NoteId> for Digest {
    fn from(value: &NoteId) -> Self {
        (*value).inner().into()
    }
}

impl From<NoteId> for Digest {
    fn from(value: NoteId) -> Self {
        value.inner().into()
    }
}

// UTILITIES
// ================================================================================================

pub fn convert<T, From, To>(from: T) -> Vec<To>
where
    T: IntoIterator<Item = From>,
    From: Into<To>,
{
    from.into_iter().map(|e| e.into()).collect()
}

pub fn try_convert<T, E, From, To>(from: T) -> Result<Vec<To>, E>
where
    T: IntoIterator<Item = From>,
    From: TryInto<To, Error = E>,
{
    from.into_iter().map(|e| e.try_into()).collect()
}

/// Given the leaf value of the nullifier SMT, returns the nullifier's block number.
///
/// There are no nullifiers in the genesis block. The value zero is instead used to signal absence
/// of a value.
pub fn nullifier_value_to_blocknum(value: Word) -> u32 {
    value[3].as_int().try_into().expect("invalid block number found in store")
}
