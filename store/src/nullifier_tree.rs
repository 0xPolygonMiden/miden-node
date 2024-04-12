use miden_objects::{
    crypto::{
        hash::rpo::RpoDigest,
        merkle::{Smt, SmtProof},
    },
    notes::Nullifier,
    Felt, FieldElement, Word,
};

use crate::{errors::NullifierTreeError, types::BlockNumber};

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

        let inner = Smt::with_entries(leaves)?;

        Ok(Self(inner))
    }

    /// Get SMT root.
    pub fn root(&self) -> RpoDigest {
        self.0.root()
    }

    /// Returns an opening of the leaf associated with the given nullifier.
    pub fn open(&self, nullifier: &Nullifier) -> SmtProof {
        self.0.open(&nullifier.inner())
    }

    /// Inserts block number in which nullifier was consumed.
    pub fn insert(
        &mut self,
        nullifier: &Nullifier,
        block_num: BlockNumber,
    ) -> Result<(), NullifierTreeError> {
        let key = nullifier.inner();
        let prev_value = self.0.get_value(&key);
        if prev_value != Smt::EMPTY_VALUE {
            return Err(NullifierTreeError::NullifierAlreadyExists {
                nullifier: *nullifier,
                block_num: Self::leaf_value_to_block_num(prev_value),
            });
        }

        self.0.insert(key, Self::block_num_to_leaf_value(block_num));

        Ok(())
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

    /// Returns the nullifier's leaf value in the SMT by its block number.
    fn block_num_to_leaf_value(block: BlockNumber) -> Word {
        [Felt::from(block), Felt::ZERO, Felt::ZERO, Felt::ZERO]
    }

    /// Given the leaf value of the nullifier SMT, returns the nullifier's block number.
    ///
    /// There are no nullifiers in the genesis block. The value zero is instead used to signal
    /// absence of a value.
    fn leaf_value_to_block_num(value: Word) -> BlockNumber {
        value[0].as_int().try_into().expect("invalid block number found in store")
    }
}

#[cfg(test)]
mod tests {
    use miden_objects::{Felt, ZERO};

    use super::NullifierTree;

    #[test]
    fn test_leaf_value_encoding() {
        let block_num = 123;
        let nullifier_value = NullifierTree::block_num_to_leaf_value(block_num);

        assert_eq!(nullifier_value, [Felt::from(block_num), ZERO, ZERO, ZERO])
    }

    #[test]
    fn test_leaf_value_decoding() {
        let block_num = 123;
        let nullifier_value = [Felt::from(block_num), ZERO, ZERO, ZERO];
        let decoded_block_num = NullifierTree::leaf_value_to_block_num(nullifier_value);

        assert_eq!(decoded_block_num, block_num);
    }
}
