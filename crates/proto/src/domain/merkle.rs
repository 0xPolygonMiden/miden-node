use miden_objects::{
    crypto::merkle::{LeafIndex, MerklePath, MmrDelta, SmtLeaf, SmtProof},
    Digest, Word,
};

use super::{convert, try_convert};
use crate::{
    errors::{ConversionError, MissingFieldHelper},
    generated,
};

// MERKLE PATH
// ================================================================================================

impl From<&MerklePath> for generated::merkle::MerklePath {
    fn from(value: &MerklePath) -> Self {
        let siblings = value.nodes().iter().map(generated::digest::Digest::from).collect();
        generated::merkle::MerklePath { siblings }
    }
}

impl From<MerklePath> for generated::merkle::MerklePath {
    fn from(value: MerklePath) -> Self {
        (&value).into()
    }
}

impl TryFrom<&generated::merkle::MerklePath> for MerklePath {
    type Error = ConversionError;

    fn try_from(merkle_path: &generated::merkle::MerklePath) -> Result<Self, Self::Error> {
        merkle_path.siblings.iter().map(Digest::try_from).collect()
    }
}

// MMR DELTA
// ================================================================================================

impl From<MmrDelta> for generated::mmr::MmrDelta {
    fn from(value: MmrDelta) -> Self {
        let data = value.data.into_iter().map(generated::digest::Digest::from).collect();
        generated::mmr::MmrDelta { forest: value.forest as u64, data }
    }
}

impl TryFrom<generated::mmr::MmrDelta> for MmrDelta {
    type Error = ConversionError;

    fn try_from(value: generated::mmr::MmrDelta) -> Result<Self, Self::Error> {
        let data: Result<Vec<_>, ConversionError> =
            value.data.into_iter().map(Digest::try_from).collect();

        Ok(MmrDelta {
            forest: value.forest as usize,
            data: data?,
        })
    }
}

// SPARSE MERKLE TREE
// ================================================================================================

// SMT LEAF
// ------------------------------------------------------------------------------------------------

impl TryFrom<generated::smt::SmtLeaf> for SmtLeaf {
    type Error = ConversionError;

    fn try_from(value: generated::smt::SmtLeaf) -> Result<Self, Self::Error> {
        let leaf = value.leaf.ok_or(generated::smt::SmtLeaf::missing_field(stringify!(leaf)))?;

        match leaf {
            generated::smt::smt_leaf::Leaf::Empty(leaf_index) => {
                Ok(Self::new_empty(LeafIndex::new_max_depth(leaf_index)))
            },
            generated::smt::smt_leaf::Leaf::Single(entry) => {
                let (key, value): (Digest, Word) = entry.try_into()?;

                Ok(SmtLeaf::new_single(key, value))
            },
            generated::smt::smt_leaf::Leaf::Multiple(entries) => {
                let domain_entries: Vec<(Digest, Word)> = try_convert(entries.entries)?;

                Ok(SmtLeaf::new_multiple(domain_entries)?)
            },
        }
    }
}

impl From<SmtLeaf> for generated::smt::SmtLeaf {
    fn from(smt_leaf: SmtLeaf) -> Self {
        use generated::smt::smt_leaf::Leaf;

        let leaf = match smt_leaf {
            SmtLeaf::Empty(leaf_index) => Leaf::Empty(leaf_index.value()),
            SmtLeaf::Single(entry) => Leaf::Single(entry.into()),
            SmtLeaf::Multiple(entries) => {
                Leaf::Multiple(generated::smt::SmtLeafEntries { entries: convert(entries) })
            },
        };

        Self { leaf: Some(leaf) }
    }
}

// SMT LEAF ENTRY
// ------------------------------------------------------------------------------------------------

impl TryFrom<generated::smt::SmtLeafEntry> for (Digest, Word) {
    type Error = ConversionError;

    fn try_from(entry: generated::smt::SmtLeafEntry) -> Result<Self, Self::Error> {
        let key: Digest = entry
            .key
            .ok_or(generated::smt::SmtLeafEntry::missing_field(stringify!(key)))?
            .try_into()?;
        let value: Word = entry
            .value
            .ok_or(generated::smt::SmtLeafEntry::missing_field(stringify!(value)))?
            .try_into()?;

        Ok((key, value))
    }
}

impl From<(Digest, Word)> for generated::smt::SmtLeafEntry {
    fn from((key, value): (Digest, Word)) -> Self {
        Self {
            key: Some(key.into()),
            value: Some(value.into()),
        }
    }
}

// SMT PROOF
// ------------------------------------------------------------------------------------------------

impl TryFrom<generated::smt::SmtOpening> for SmtProof {
    type Error = ConversionError;

    fn try_from(opening: generated::smt::SmtOpening) -> Result<Self, Self::Error> {
        let path: MerklePath = opening
            .path
            .as_ref()
            .ok_or(generated::smt::SmtOpening::missing_field(stringify!(path)))?
            .try_into()?;
        let leaf: SmtLeaf = opening
            .leaf
            .ok_or(generated::smt::SmtOpening::missing_field(stringify!(leaf)))?
            .try_into()?;

        Ok(SmtProof::new(path, leaf)?)
    }
}

impl From<SmtProof> for generated::smt::SmtOpening {
    fn from(proof: SmtProof) -> Self {
        let (ref path, leaf) = proof.into_parts();
        Self {
            path: Some(path.into()),
            leaf: Some(leaf.into()),
        }
    }
}
