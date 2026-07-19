use std::sync::Arc;

use super::nar_transfer::NarTransferMetric;

#[derive(Debug, Clone, Default)]
pub struct MetricStore {
    pub nar_transfer: Arc<NarTransferMetric>,
}

impl MetricStore {
    pub fn new() -> Self {
        Self {
            nar_transfer: Arc::new(NarTransferMetric::new()),
        }
    }
}
