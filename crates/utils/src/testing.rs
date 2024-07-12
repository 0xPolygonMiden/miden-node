use miden_objects::{crypto::hash::rpo::RpoDigest, notes::Nullifier, Felt, Word, ZERO};

pub fn num_to_rpo_digest(n: u64) -> RpoDigest {
    RpoDigest::new(num_to_word(n))
}

pub fn num_to_word(n: u64) -> Word {
    [ZERO, ZERO, ZERO, Felt::new(n)]
}

pub fn num_to_nullifier(n: u64) -> Nullifier {
    Nullifier::from(num_to_rpo_digest(n))
}
