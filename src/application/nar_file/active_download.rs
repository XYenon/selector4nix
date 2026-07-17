use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context, Poll};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result as AnyhowResult;
use bytes::Bytes;
use dashmap::DashMap;
use futures::Stream;

use crate::domain::common::store_path::store_path_name;
use crate::domain::common::url::Url;
use crate::domain::nar_file::model::NarFileKey;
use crate::domain::nar_file::port::NarStreamData;
use crate::domain::substituter::model::SubstituterMeta;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct DownloadId(u64);

#[derive(Debug)]
struct ActiveDownloadEntry {
    /// Package name from StorePath when known, e.g. `codex-0.144.5`.
    name: String,
    /// NAR file name, e.g. `abc.nar.xz`.
    file: String,
    substituter_url: Url,
    source_url: Url,
    content_length: Option<u64>,
    bytes_transferred: Arc<AtomicU64>,
    started_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveDownloadSnapshot {
    pub id: u64,
    pub name: String,
    pub file: String,
    pub substituter: String,
    pub source_url: String,
    pub content_length: Option<u64>,
    pub bytes_transferred: u64,
    pub started_at_unix_ms: u64,
}

/// Tracks in-flight NAR transfers for the status dashboard.
#[derive(Debug, Default)]
pub struct ActiveDownloadRegistry {
    next_id: AtomicU64,
    entries: DashMap<DownloadId, ActiveDownloadEntry>,
}

impl ActiveDownloadRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn track(
        self: &Arc<Self>,
        key: &NarFileKey,
        substituter: &SubstituterMeta,
        source_url: &Url,
        content_length: Option<u64>,
        store_path: Option<&str>,
    ) -> ActiveDownloadGuard {
        let id = DownloadId(self.next_id.fetch_add(1, Ordering::Relaxed));
        let bytes_transferred = Arc::new(AtomicU64::new(0));
        let started_at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let file = key.to_file_name().value().to_string();
        let name = store_path
            .and_then(store_path_name)
            .map(str::to_string)
            .unwrap_or_else(|| file.clone());

        self.entries.insert(
            id,
            ActiveDownloadEntry {
                name,
                file,
                substituter_url: substituter.url().clone(),
                source_url: source_url.clone(),
                content_length,
                bytes_transferred: bytes_transferred.clone(),
                started_at_unix_ms,
            },
        );

        ActiveDownloadGuard {
            registry: Arc::clone(self),
            id,
            bytes_transferred,
        }
    }

    pub fn list(&self) -> Vec<ActiveDownloadSnapshot> {
        let mut items: Vec<ActiveDownloadSnapshot> = self
            .entries
            .iter()
            .map(|entry| {
                let id = entry.key().0;
                let value = entry.value();
                ActiveDownloadSnapshot {
                    id,
                    name: value.name.clone(),
                    file: value.file.clone(),
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

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn remove(&self, id: DownloadId) {
        self.entries.remove(&id);
    }
}

pub struct ActiveDownloadGuard {
    registry: Arc<ActiveDownloadRegistry>,
    id: DownloadId,
    bytes_transferred: Arc<AtomicU64>,
}

impl ActiveDownloadGuard {
    fn record_bytes(&self, n: u64) {
        self.bytes_transferred.fetch_add(n, Ordering::Relaxed);
    }
}

impl Drop for ActiveDownloadGuard {
    fn drop(&mut self) {
        self.registry.remove(self.id);
    }
}

struct TrackedNarStream {
    inner: Pin<Box<dyn Stream<Item = AnyhowResult<Bytes>> + Send>>,
    guard: ActiveDownloadGuard,
}

impl Stream for TrackedNarStream {
    type Item = AnyhowResult<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                self.guard.record_bytes(bytes.len() as u64);
                Poll::Ready(Some(Ok(bytes)))
            }
            other => other,
        }
    }
}

pub fn track_stream(
    registry: &Arc<ActiveDownloadRegistry>,
    key: &NarFileKey,
    data: NarStreamData,
    store_path: Option<&str>,
) -> NarStreamData {
    let NarStreamData {
        headers,
        inner,
        source_url,
        substituter,
        store_path_hash,
    } = data;

    let guard = registry.track(
        key,
        &substituter,
        &source_url,
        headers.content_length,
        store_path,
    );
    let tracked = TrackedNarStream { inner, guard };

    NarStreamData::new(headers, Box::pin(tracked), source_url, substituter)
        .with_store_path_hash(store_path_hash)
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;

    use crate::domain::common::url::Url;
    use crate::domain::nar_file::port::NarStreamHeaders;
    use crate::domain::nar_info::model::NarFileName;
    use crate::domain::substituter::model::{Priority, SubstituterMeta};

    use super::*;

    fn sample_key() -> NarFileKey {
        let name =
            NarFileName::new("1w1fff338fvdw53sqgamddn1b2xgds473pv6y13gizdbqjv4i5p3.nar.xz".into())
                .unwrap();
        NarFileKey::from_file_name(&name)
    }

    fn sample_meta() -> SubstituterMeta {
        SubstituterMeta::new(
            Url::new("https://cache.nixos.org/").unwrap(),
            Priority::new(40).unwrap(),
        )
    }

    #[tokio::test]
    async fn track_stream_labels_counts_bytes_and_clears_on_drop() {
        let registry = Arc::new(ActiveDownloadRegistry::new());
        let key = sample_key();
        let meta = sample_meta();
        let source = Url::new(
            "https://cache.nixos.org/nar/1w1fff338fvdw53sqgamddn1b2xgds473pv6y13gizdbqjv4i5p3.nar.xz",
        )
        .unwrap();

        let data = NarStreamData::new(
            NarStreamHeaders {
                content_length: Some(5),
                content_type: None,
                content_encoding: None,
            },
            Box::pin(futures::stream::iter(vec![
                Ok(Bytes::from_static(b"ab")),
                Ok(Bytes::from_static(b"cde")),
            ])),
            source,
            meta,
        );

        let mut tracked = track_stream(
            &registry,
            &key,
            data,
            Some("/nix/store/zj64jfhbxbync50az13gxr6k7bnqhcb3-codex-0.144.5"),
        );
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.list()[0].name, "codex-0.144.5");
        assert_eq!(
            registry.list()[0].file,
            "1w1fff338fvdw53sqgamddn1b2xgds473pv6y13gizdbqjv4i5p3.nar.xz"
        );

        assert_eq!(tracked.inner.next().await.unwrap().unwrap().as_ref(), b"ab");
        assert_eq!(registry.list()[0].bytes_transferred, 2);

        assert_eq!(
            tracked.inner.next().await.unwrap().unwrap().as_ref(),
            b"cde"
        );
        assert_eq!(registry.list()[0].bytes_transferred, 5);
        assert!(tracked.inner.next().await.is_none());

        drop(tracked);
        assert!(registry.is_empty());
    }
}
