pub mod config;
pub mod server;

#[macro_export]
macro_rules! target {
    () => {
        "miden-rpc"
    };
}

// CONSTANTS
// =================================================================================================
pub const COMPONENT: &str = target!();
