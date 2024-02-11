use miden_objects::{StarkField, Word};

pub mod accounts;
pub mod blocks;
pub mod digest;
pub mod merkle;
pub mod notes;
pub mod nullifiers;
pub mod transactions;

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
pub fn nullifier_value_to_block_num(value: Word) -> u32 {
    value[3].as_int().try_into().expect("invalid block number found in store")
}
