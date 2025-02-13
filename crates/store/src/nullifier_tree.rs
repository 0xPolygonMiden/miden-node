use miden_objects::{
    block::BlockNumber,
    crypto::{
        hash::rpo::RpoDigest,
        merkle::{MutationSet, Smt, SmtProof, SMT_DEPTH},
    },
    note::Nullifier,
    Felt, FieldElement, Word,
};

use crate::errors::NullifierTreeError;

/// Nullifier SMT.
#[derive(Debug, Clone)]
pub struct NullifierTree(Smt);

impl NullifierTree {
    /// Construct new nullifier tree from list of items.
    pub fn with_entries(
        entries: impl IntoIterator<Item = (Nullifier, BlockNumber)>,
    ) -> Result<Self, NullifierTreeError> {
        let leaves = entries.into_iter().map(|(nullifier, block_num)| {
            (nullifier.inner(), Self::block_num_to_leaf_value(block_num))
        });

        let inner = Smt::with_entries(leaves).map_err(NullifierTreeError::CreationFailed)?;

        Ok(Self(inner))
    }

    /// Returns the root of the nullifier SMT.
    pub fn root(&self) -> RpoDigest {
        self.0.root()
    }

    /// Returns an opening of the leaf associated with the given nullifier.
    pub fn open(&self, nullifier: &Nullifier) -> SmtProof {
        self.0.open(&nullifier.inner())
    }

    /// Returns block number stored for the given nullifier or `None` if the nullifier wasn't
    /// consumed.
    pub fn get_block_num(&self, nullifier: &Nullifier) -> Option<BlockNumber> {
        let value = self.0.get_value(&nullifier.inner());
        if value == Smt::EMPTY_VALUE {
            return None;
        }

        Some(Self::leaf_value_to_block_num(value))
    }

    /// Computes mutations for the nullifier SMT.
    pub fn compute_mutations(
        &self,
        kv_pairs: impl IntoIterator<Item = (Nullifier, BlockNumber)>,
    ) -> MutationSet<SMT_DEPTH, RpoDigest, Word> {
        self.0.compute_mutations(kv_pairs.into_iter().map(|(nullifier, block_num)| {
            (nullifier.inner(), Self::block_num_to_leaf_value(block_num))
        }))
    }

    /// Applies mutations to the nullifier SMT.
    pub fn apply_mutations(
        &mut self,
        mutations: MutationSet<SMT_DEPTH, RpoDigest, Word>,
    ) -> Result<(), NullifierTreeError> {
        self.0.apply_mutations(mutations).map_err(NullifierTreeError::MutationFailed)
    }

    // HELPER FUNCTIONS
    // --------------------------------------------------------------------------------------------

    /// Returns the nullifier's leaf value in the SMT by its block number.
    fn block_num_to_leaf_value(block: BlockNumber) -> Word {
        [Felt::from(block), Felt::ZERO, Felt::ZERO, Felt::ZERO]
    }

    /// Given the leaf value of the nullifier SMT, returns the nullifier's block number.
    ///
    /// There are no nullifiers in the genesis block. The value zero is instead used to signal
    /// absence of a value.
    fn leaf_value_to_block_num(value: Word) -> BlockNumber {
        let block_num: u32 =
            value[0].as_int().try_into().expect("invalid block number found in store");

        block_num.into()
    }
}

#[cfg(test)]
mod tests {
    use miden_objects::{Felt, ZERO};

    use super::NullifierTree;

    #[test]
    fn leaf_value_encoding() {
        let block_num = 123;
        let nullifier_value = NullifierTree::block_num_to_leaf_value(block_num.into());

        assert_eq!(nullifier_value, [Felt::from(block_num), ZERO, ZERO, ZERO]);
    }

    #[test]
    fn leaf_value_decoding() {
        let block_num = 123;
        let nullifier_value = [Felt::from(block_num), ZERO, ZERO, ZERO];
        let decoded_block_num = NullifierTree::leaf_value_to_block_num(nullifier_value);

        assert_eq!(decoded_block_num, block_num.into());
    }
}
