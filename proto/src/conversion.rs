use crate::{account_id, block_header, digest, error, merkle, mmr, note, responses, tsmt};
use miden_crypto::{
    hash::rpo::RpoDigest,
    merkle::{MerklePath, MmrDelta, TieredSmtProof},
    Felt, FieldElement, StarkField, Word,
};
use miden_objects::BlockHeader;

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

impl From<TieredSmtProof> for tsmt::NullifierProof {
    fn from(value: TieredSmtProof) -> Self {
        let (path, entries) = value.into_parts();

        tsmt::NullifierProof {
            merkle_path: convert(path),
            leaves: convert(entries),
        }
    }
}

impl From<(RpoDigest, Word)> for tsmt::NullifierLeaf {
    fn from(value: (RpoDigest, Word)) -> Self {
        let (key, value) = value;
        Self {
            key: Some(key.into()),
            block_num: nullifier_value_to_blocknum(value),
        }
    }
}

impl TryFrom<tsmt::NullifierProof> for TieredSmtProof {
    type Error = error::ParseError;

    fn try_from(value: tsmt::NullifierProof) -> Result<Self, Self::Error> {
        let path = MerklePath::new(try_convert(value.merkle_path)?);
        let entries = value
            .leaves
            .into_iter()
            .map(|leaf| {
                let key = leaf.key.ok_or(error::ParseError::MissingLeafKey)?.try_into()?;
                let value = [Felt::ZERO, Felt::ZERO, Felt::ZERO, Felt::from(leaf.block_num)];
                let result = (key, value);

                Ok(result)
            })
            .collect::<Result<Vec<(RpoDigest, Word)>, Self::Error>>()?;
        TieredSmtProof::new(path, entries).or(Err(error::ParseError::InvalidProof))
    }
}

impl TryFrom<digest::Digest> for [Felt; 4] {
    type Error = error::ParseError;

    fn try_from(value: digest::Digest) -> Result<Self, Self::Error> {
        if ![value.d0, value.d1, value.d2, value.d3]
            .iter()
            .all(|v| *v < <Felt as StarkField>::MODULUS)
        {
            Err(error::ParseError::NotAValidFelt)
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
    type Error = error::ParseError;

    fn try_from(value: digest::Digest) -> Result<Self, Self::Error> {
        Ok(Self::new(value.try_into()?))
    }
}

impl TryFrom<&digest::Digest> for [Felt; 4] {
    type Error = error::ParseError;

    fn try_from(value: &digest::Digest) -> Result<Self, Self::Error> {
        value.clone().try_into()
    }
}

impl TryFrom<&digest::Digest> for RpoDigest {
    type Error = error::ParseError;

    fn try_from(value: &digest::Digest) -> Result<Self, Self::Error> {
        value.clone().try_into()
    }
}

impl TryFrom<block_header::BlockHeader> for BlockHeader {
    type Error = error::ParseError;

    fn try_from(value: block_header::BlockHeader) -> Result<Self, Self::Error> {
        Ok(BlockHeader::new(
            value.prev_hash.ok_or(error::ParseError::ProtobufMissingData)?.try_into()?,
            value.block_num.into(),
            value.chain_root.ok_or(error::ParseError::ProtobufMissingData)?.try_into()?,
            value.account_root.ok_or(error::ParseError::ProtobufMissingData)?.try_into()?,
            value.nullifier_root.ok_or(error::ParseError::ProtobufMissingData)?.try_into()?,
            value.note_root.ok_or(error::ParseError::ProtobufMissingData)?.try_into()?,
            value.batch_root.ok_or(error::ParseError::ProtobufMissingData)?.try_into()?,
            value.proof_hash.ok_or(error::ParseError::ProtobufMissingData)?.try_into()?,
            value.version.into(),
            value.timestamp.into(),
        ))
    }
}

impl TryFrom<&block_header::BlockHeader> for BlockHeader {
    type Error = error::ParseError;

    fn try_from(value: &block_header::BlockHeader) -> Result<Self, Self::Error> {
        value.clone().try_into()
    }
}

impl TryFrom<mmr::MmrDelta> for MmrDelta {
    type Error = error::ParseError;

    fn try_from(value: mmr::MmrDelta) -> Result<Self, Self::Error> {
        let data: Result<Vec<RpoDigest>, error::ParseError> =
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

impl From<note::Note> for responses::NoteSyncRecord {
    fn from(value: note::Note) -> Self {
        Self {
            note_index: value.note_index,
            note_hash: value.note_hash,
            sender: value.sender,
            tag: value.tag,
            num_assets: value.num_assets,
            merkle_path: value.merkle_path,
        }
    }
}

impl From<account_id::AccountId> for u64 {
    fn from(value: account_id::AccountId) -> Self {
        value.id
    }
}

impl From<u64> for account_id::AccountId {
    fn from(value: u64) -> Self {
        account_id::AccountId { id: value }
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

/// Given the leaf value of the nullifier TSMT, returns the nullifier's block number.
///
/// There are no nullifiers in the genesis block. The value zero is instead used to signal absence
/// of a value.
pub fn nullifier_value_to_blocknum(value: Word) -> u32 {
    value[3].as_int().try_into().expect("invalid block number found in store")
}
