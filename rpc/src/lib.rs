pub mod config;
pub mod server;

#[macro_export]
macro_rules! target {
    () => {
        "miden-rpc"
    };
}
