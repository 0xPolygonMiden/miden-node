use miden_crypto::{
    merkle::{LeafIndex, MerklePath, SmtLeaf, SmtProof},
    Word,
};
use miden_objects::{Digest, Digest as RpoDigest};

use crate::{
    domain::{convert, try_convert},
    errors::{MissingFieldHelper, ParseError},
    generated::{responses::NullifierBlockInputRecord, smt},
};

// NULLIFIER INPUT RECORD
// ================================================================================================

#[derive(Clone, Debug)]
pub struct NullifierInputRecord {
    pub nullifier: Digest,
    pub proof: SmtProof,
}

impl TryFrom<NullifierBlockInputRecord> for NullifierInputRecord {
    type Error = ParseError;

    fn try_from(nullifier_input_record: NullifierBlockInputRecord) -> Result<Self, Self::Error> {
        Ok(Self {
            nullifier: nullifier_input_record
                .nullifier
                .ok_or(NullifierBlockInputRecord::missing_field(stringify!(nullifier)))?
                .try_into()?,
            proof: nullifier_input_record
                .opening
                .ok_or(NullifierBlockInputRecord::missing_field(stringify!(opening)))?
                .try_into()?,
        })
    }
}

impl From<NullifierInputRecord> for NullifierBlockInputRecord {
    fn from(value: NullifierInputRecord) -> Self {
        Self {
            nullifier: Some(value.nullifier.into()),
            opening: Some(value.proof.into()),
        }
    }
}

impl TryFrom<smt::SmtLeaf> for SmtLeaf {
    type Error = ParseError;

    fn try_from(value: smt::SmtLeaf) -> Result<Self, Self::Error> {
        let leaf = value.leaf.ok_or(smt::SmtLeaf::missing_field(stringify!(leaf)))?;

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
    type Error = ParseError;

    fn try_from(entry: smt::SmtLeafEntry) -> Result<Self, Self::Error> {
        let key: RpoDigest =
            entry.key.ok_or(smt::SmtLeafEntry::missing_field(stringify!(key)))?.try_into()?;
        let value: Word = entry
            .value
            .ok_or(smt::SmtLeafEntry::missing_field(stringify!(value)))?
            .try_into()?;

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
    type Error = ParseError;

    fn try_from(opening: smt::SmtOpening) -> Result<Self, Self::Error> {
        let path: MerklePath = opening
            .path
            .ok_or(smt::SmtOpening::missing_field(stringify!(path)))?
            .try_into()?;
        let leaf: SmtLeaf = opening
            .leaf
            .ok_or(smt::SmtOpening::missing_field(stringify!(leaf)))?
            .try_into()?;

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
