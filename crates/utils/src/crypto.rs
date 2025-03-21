use miden_objects::{
    Felt,
    crypto::{hash::rpo::RpoDigest, rand::RpoRandomCoin},
};
use rand::Rng;

/// Creates a new RPO Random Coin with random seed
pub fn get_rpo_random_coin<T: Rng>(rng: &mut T) -> RpoRandomCoin {
    let auth_seed: [u64; 4] = rng.random();
    let rng_seed = RpoDigest::from(auth_seed.map(Felt::new));

    RpoRandomCoin::new(rng_seed.into())
}
