pub mod config;
pub mod genesis;
pub mod logging;

// RE-EXPORTS
// ================================================================================================
pub use config::Config;

// CONSTANTS
// ================================================================================================

/// The name of the organization - for config file path purposes
pub const ORG: &str = "Polygon";
/// The name of the app - for config file path purposes
pub const APP: &str = "Miden";
