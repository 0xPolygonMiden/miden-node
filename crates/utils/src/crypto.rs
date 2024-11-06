use miden_objects::{
    crypto::{hash::rpo::RpoDigest, rand::RpoRandomCoin},
    Felt,
};
use rand::{Rng, RngCore};

/// Creates a new RPO Random Coin with random seed
pub fn get_rpo_random_coin<T: RngCore>(rng: &mut T) -> RpoRandomCoin {
    let auth_seed: [u64; 4] = rng.gen();
    let rng_seed = RpoDigest::from(auth_seed.map(Felt::new));

    RpoRandomCoin::new(rng_seed.into())
}
