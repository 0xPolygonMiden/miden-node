/// Token symbol of the faucet present at genesis
pub const FUNGIBLE_FAUCET_TOKEN_SYMBOL: &str = "POL";

/// Decimals for the token of the faucet present at genesis
pub const FUNGIBLE_FAUCET_TOKEN_DECIMALS: u8 = 9;

/// Max supply for the token of the faucet present at genesis
pub const FUNGIBLE_FAUCET_TOKEN_MAX_SUPPLY: u64 = 1_000_000_000;

/// Seed for the Falcon512 keypair (faucet account)
pub const SEED_FAUCET_KEYPAIR: [u8; 40] = [2_u8; 40];

/// Seed for the Falcon512 keypair (wallet account)
pub const SEED_WALLET_KEYPAIR: [u8; 40] = [3_u8; 40];

/// Seed for the fungible faucet account
pub const SEED_FAUCET: [u8; 32] = [0_u8; 32];

/// Seed for the basic wallet account
pub const SEED_WALLET: [u8; 32] = [1_u8; 32];

/// Faucet account keys (public/private) file path
pub const FAUCET_KEYPAIR_FILE_PATH: &str = "faucet.fsk";

/// Wallet account keys (public/private) file path
pub const WALLET_KEYPAIR_FILE_PATH: &str = "wallet.fsk";
