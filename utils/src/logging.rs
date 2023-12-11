use anyhow::Result;
use tracing::{debug, subscriber};
use tracing_subscriber;

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
        .finish();
    subscriber::set_global_default(subscriber)?;

    Ok(())
}
