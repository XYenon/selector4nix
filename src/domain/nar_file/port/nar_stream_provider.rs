use std::pin::Pin;

use anyhow::Result as AnyhowResult;
use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;

use crate::domain::common::passthrough_headers::PassthroughHeaders;
use crate::domain::common::url::Url;
use crate::domain::nar_file::model::NarFileLocation;
use crate::domain::nar_info::model::StorePathHash;
use crate::domain::substituter::model::SubstituterMeta;

#[async_trait]
pub trait NarStreamProvider: Send + Sync {
    async fn stream_nar(
        &self,
        locations: &[NarFileLocation],
        headers: &PassthroughHeaders,
    ) -> AnyhowResult<Option<NarStreamData>>;
}

pub struct NarStreamData {
    pub headers: NarStreamHeaders,
    pub inner: Pin<Box<dyn Stream<Item = AnyhowResult<Bytes>> + Send>>,
    pub source_url: Url,
    pub substituter: SubstituterMeta,
    /// Reverse link to `NarInfo` when known from a prior narinfo resolution.
    pub store_path_hash: Option<StorePathHash>,
}

impl NarStreamData {
    pub fn new(
        headers: NarStreamHeaders,
        inner: Pin<Box<dyn Stream<Item = AnyhowResult<Bytes>> + Send>>,
        source_url: Url,
        substituter: SubstituterMeta,
    ) -> Self {
        Self {
            headers,
            inner,
            source_url,
            substituter,
            store_path_hash: None,
        }
    }

    pub fn with_store_path_hash(mut self, store_path_hash: Option<StorePathHash>) -> Self {
        self.store_path_hash = store_path_hash;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NarStreamHeaders {
    pub content_length: Option<u64>,
    pub content_type: Option<String>,
    pub content_encoding: Option<String>,
}
