//! In-process NAR transfer metrics.
//!
//! Write path: `begin` -> [`NarTransferHandle::record_bytes`] -> drop handle.
//! Read path: [`NarTransferMetric::active`].

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use dashmap::DashMap;

use crate::domain::common::url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct TransferId(u64);

#[derive(Debug, Clone)]
pub struct NarTransferAttrs {
    pub nar_file: String,
    pub store_path: Option<String>,
    pub substituter_url: Url,
    pub source_url: Url,
    pub content_length: Option<u64>,
}

#[derive(Debug)]
struct NarTransferEntry {
    store_path: Option<String>,
    nar_file: String,
    substituter_url: Url,
    source_url: Url,
    content_length: Option<u64>,
    bytes_transferred: Arc<AtomicU64>,
    started_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NarTransferSample {
    pub id: u64,
    pub store_path: Option<String>,
    pub nar_file: String,
    pub substituter: String,
    pub source_url: String,
    pub content_length: Option<u64>,
    pub bytes_transferred: u64,
    pub started_at_unix_ms: u64,
}

#[derive(Debug, Default)]
pub struct NarTransferMetric {
    next_id: AtomicU64,
    entries: DashMap<TransferId, NarTransferEntry>,
}

impl NarTransferMetric {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin(self: &Arc<Self>, attrs: NarTransferAttrs) -> NarTransferHandle {
        let id = TransferId(self.next_id.fetch_add(1, Ordering::Relaxed));
        let bytes_transferred = Arc::new(AtomicU64::new(0));
        let started_at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        self.entries.insert(
            id,
            NarTransferEntry {
                store_path: attrs.store_path,
                nar_file: attrs.nar_file,
                substituter_url: attrs.substituter_url,
                source_url: attrs.source_url,
                content_length: attrs.content_length,
                bytes_transferred: bytes_transferred.clone(),
                started_at_unix_ms,
            },
        );

        NarTransferHandle {
            metric: Arc::clone(self),
            id,
            bytes_transferred,
        }
    }

    pub fn active(&self) -> Vec<NarTransferSample> {
        let mut items: Vec<NarTransferSample> = self
            .entries
            .iter()
            .map(|entry| {
                let id = entry.key().0;
                let value = entry.value();
                NarTransferSample {
                    id,
                    store_path: value.store_path.clone(),
                    nar_file: value.nar_file.clone(),
                    substituter: value.substituter_url.to_string(),
                    source_url: value.source_url.to_string(),
                    content_length: value.content_length,
                    bytes_transferred: value.bytes_transferred.load(Ordering::Relaxed),
                    started_at_unix_ms: value.started_at_unix_ms,
                }
            })
            .collect();
        items.sort_by_key(|item| item.started_at_unix_ms);
        items
    }

    pub fn active_count(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn remove(&self, id: TransferId) {
        self.entries.remove(&id);
    }
}

pub struct NarTransferHandle {
    metric: Arc<NarTransferMetric>,
    id: TransferId,
    bytes_transferred: Arc<AtomicU64>,
}

impl NarTransferHandle {
    pub fn record_bytes(&self, n: u64) {
        self.bytes_transferred.fetch_add(n, Ordering::Relaxed);
    }
}

impl Drop for NarTransferHandle {
    fn drop(&mut self) {
        self.metric.remove(self.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::common::url::Url;

    #[test]
    fn begin_records_active_sample_and_drop_clears() {
        let metric = Arc::new(NarTransferMetric::new());
        let store_path = "/nix/store/zj64jfhbxbync50az13gxr6k7bnqhcb3-codex-0.144.5".to_string();
        let handle = metric.begin(NarTransferAttrs {
            nar_file: "abc.nar.xz".into(),
            store_path: Some(store_path.clone()),
            substituter_url: Url::new("https://cache.nixos.org/").unwrap(),
            source_url: Url::new("https://cache.nixos.org/nar/abc.nar.xz").unwrap(),
            content_length: Some(10),
        });

        assert_eq!(metric.active_count(), 1);
        assert_eq!(
            metric.active()[0].store_path.as_deref(),
            Some(store_path.as_str())
        );
        handle.record_bytes(4);
        assert_eq!(metric.active()[0].bytes_transferred, 4);

        drop(handle);
        assert!(metric.is_empty());
    }
}
