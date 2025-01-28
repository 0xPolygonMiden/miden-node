use std::{hint::black_box, io::Write, mem::swap, time::Instant};

use miden_crypto::{merkle::LeafIndex, Felt, Word};
use miden_node_store::AccountTree;
use miden_objects::block::BlockNumber;
use rand::random;
use winter_rand_utils::prng_array;

const ACCOUNT_NUMBER: usize = 10_000;
const LAST_BLOCK_NUMBER: usize = 100;
const UPDATES_PER_BLOCK: usize = 1_000;

#[tokio::main]
async fn main() {
    print!("Preparing ({LAST_BLOCK_NUMBER} blocks)... ");
    std::io::stdout().flush().unwrap();
    let mut seed = [0u8; 32];

    let mut tree = AccountTree::default();

    let mut leaf_indexes = Vec::with_capacity(ACCOUNT_NUMBER);
    let mut init_account_updates = Vec::with_capacity(ACCOUNT_NUMBER);
    for _ in 0..ACCOUNT_NUMBER {
        let id: u64 = random();
        let leaf_index = LeafIndex::new(id).unwrap();
        leaf_indexes.push(leaf_index);
        init_account_updates.push((leaf_index, generate_word(&mut seed)));
    }

    let mutations = tree.accounts().compute_mutations(init_account_updates);
    tree.apply_mutations(1.into(), mutations).await.unwrap();
    print!("1 ");
    std::io::stdout().flush().unwrap();

    for block_num in 2..=LAST_BLOCK_NUMBER {
        let block_num = BlockNumber::from_usize(block_num);
        let mut account_updates = Vec::with_capacity(UPDATES_PER_BLOCK);
        for _ in 0..UPDATES_PER_BLOCK {
            let leaf_index = leaf_indexes[random::<usize>() % leaf_indexes.len()];
            account_updates.push((leaf_index, generate_word(&mut seed)));
        }
        let mutations = tree.accounts().compute_mutations(account_updates);
        tree.apply_mutations(block_num, mutations).await.unwrap();
        print!("{block_num} ");
        std::io::stdout().flush().unwrap();
    }

    println!("Done");
    std::io::stdout().flush().unwrap();

    let now = Instant::now();

    for _block_num in 1..LAST_BLOCK_NUMBER {
        for leaf_index in &leaf_indexes {
            let _opening = black_box(tree.accounts().open(leaf_index));
        }
    }

    let elapsed_avg = now.elapsed().as_millis() / (LAST_BLOCK_NUMBER - 1) as u128;

    println!("SMT opening elapsed (for {ACCOUNT_NUMBER} accounts, average): {elapsed_avg} ms");
    std::io::stdout().flush().unwrap();

    let now = Instant::now();

    for block_num in 1..LAST_BLOCK_NUMBER {
        let block_num = BlockNumber::from_usize(block_num);
        for leaf_index in &leaf_indexes {
            let _opening = black_box(tree.compute_opening(leaf_index.value(), block_num).unwrap());
        }
    }

    let elapsed_avg = now.elapsed().as_millis() / (LAST_BLOCK_NUMBER - 1) as u128;

    println!("AccountTree `compute_opening` elapsed (for {ACCOUNT_NUMBER} accounts, average): {elapsed_avg} ms");
}

fn generate_word(seed: &mut [u8; 32]) -> Word {
    swap(seed, &mut prng_array(*seed));
    let nums: [u64; 4] = prng_array(*seed);
    [Felt::new(nums[0]), Felt::new(nums[1]), Felt::new(nums[2]), Felt::new(nums[3])]
}
