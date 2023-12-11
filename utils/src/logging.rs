use anyhow::Result;
use tracing::{debug, subscriber};
use tracing_subscriber::{self, EnvFilter};

pub fn setup_logging() -> Result<()> {
    let subscriber = tracing_subscriber::fmt()
        .json()
        .compact()
        .with_level(true)
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(false)
        .with_thread_names(true)
        .with_target(true)
        .with_env_filter(EnvFilter::from_default_env())
        .finish();
    subscriber::set_global_default(subscriber)?;

    Ok(())
}
