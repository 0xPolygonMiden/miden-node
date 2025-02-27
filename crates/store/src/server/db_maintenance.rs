use std::{sync::Arc, time::Duration};

use tracing::{error, info};

use crate::{state::State, COMPONENT};

pub struct DbMaintenance {
    state: Arc<State>,
    optimization_interval: Duration,
}

impl DbMaintenance {
    pub fn new(state: Arc<State>, optimization_interval: Duration) -> Self {
        Self { state, optimization_interval }
    }

    /// Runs infinite maintenance loop.
    pub async fn run(self) {
        loop {
            tokio::time::sleep(self.optimization_interval).await;

            info!(target: COMPONENT, "Starting database optimization");

            match self.state.optimize_db().await {
                Ok(_) => info!(target: COMPONENT, "Finished database optimization"),
                Err(err) => error!(target: COMPONENT, %err, "Database optimization failed"),
            }
        }
    }
}
