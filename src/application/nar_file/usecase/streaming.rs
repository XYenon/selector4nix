use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use anyhow::Result as AnyhowResult;
use bytes::Bytes;
use futures::Stream;

use crate::application::nar_file::actor::{NarFileActorRegistry, NarFileRequest};
use crate::domain::common::passthrough_headers::PassthroughHeaders;
use crate::domain::nar_file::model::NarFileKey;
use crate::domain::nar_file::port::NarStreamData;
use crate::domain::nar_info::NarInfoRepository;
use crate::domain::nar_info::model::StorePathHash;
use crate::infrastructure::metric::{NarTransferAttrs, NarTransferHandle, NarTransferMetric};
use crate::{AppError, AppResultExt};

pub struct NarFileStreamingUseCase {
    nar_file_registry: Arc<NarFileActorRegistry>,
    nar_info_repository: Arc<dyn NarInfoRepository>,
    nar_transfer_metric: Arc<NarTransferMetric>,
}

impl NarFileStreamingUseCase {
    pub fn new(
        nar_file_registry: Arc<NarFileActorRegistry>,
        nar_info_repository: Arc<dyn NarInfoRepository>,
        nar_transfer_metric: Arc<NarTransferMetric>,
    ) -> Self {
        Self {
            nar_file_registry,
            nar_info_repository,
            nar_transfer_metric,
        }
    }

    pub async fn stream_nar(
        &self,
        key: NarFileKey,
        headers: PassthroughHeaders,
    ) -> Result<NarStreamData, AppError> {
        tracing::info!(nar_file = %key.to_file_name().value(), "acquiring nar stream from substituter");

        let address = self.nar_file_registry.get(&key).await;

        let response = address
            .ask(|reply_to| NarFileRequest::StreamNarFile { reply_to, headers })
            .await
            .throw_catastrophic("`NarFileActor` terminated unexpectedly")?;

        let result = response
            .inspect(|result| tracing::info!(nar_file = %key.to_file_name().value(), source_url = %result.stream.source_url, substituter = %result.stream.substituter.url(), "streamed nar from substituter"))
            .inspect_err(|err| tracing::warn!(nar_file = %key.to_file_name().value(), %err, "failed to stream nar"))?;

        let store_path = self.query_store_path(result.store_path_hash.as_ref()).await;

        let attrs = NarTransferAttrs {
            nar_file: key.to_file_name().value().to_string(),
            store_path,
            substituter_url: result.stream.substituter.url().clone(),
            source_url: result.stream.source_url.clone(),
            content_length: result.stream.headers.content_length,
        };

        Ok(instrument_stream(
            &self.nar_transfer_metric,
            attrs,
            result.stream,
        ))
    }

    async fn query_store_path(&self, hash: Option<&StorePathHash>) -> Option<String> {
        let hash = hash?;
        let nar_info = self.nar_info_repository.get(hash).await.ok().flatten()?;
        nar_info
            .nar_info()
            .and_then(|data| data.store_path().map(str::to_string))
    }
}

fn instrument_stream(
    metric: &Arc<NarTransferMetric>,
    attrs: NarTransferAttrs,
    data: NarStreamData,
) -> NarStreamData {
    let NarStreamData {
        headers,
        inner,
        source_url,
        substituter,
    } = data;

    let handle = metric.begin(attrs);
    let instrumented = InstrumentedNarStream { inner, handle };

    NarStreamData::new(headers, Box::pin(instrumented), source_url, substituter)
}

struct InstrumentedNarStream {
    inner: Pin<Box<dyn Stream<Item = AnyhowResult<Bytes>> + Send>>,
    handle: NarTransferHandle,
}

impl Stream for InstrumentedNarStream {
    type Item = AnyhowResult<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                self.handle.record_bytes(bytes.len() as u64);
                Poll::Ready(Some(Ok(bytes)))
            }
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;

    use crate::domain::common::url::Url;
    use crate::domain::nar_file::port::NarStreamHeaders;
    use crate::domain::substituter::model::{Priority, SubstituterMeta};

    use super::*;

    fn sample_meta() -> SubstituterMeta {
        SubstituterMeta::new(
            Url::new("https://cache.nixos.org/").unwrap(),
            Priority::new(40).unwrap(),
        )
    }

    #[tokio::test]
    async fn instrument_stream_counts_bytes_and_clears_on_drop() {
        let metric = Arc::new(NarTransferMetric::new());
        let meta = sample_meta();
        let source = Url::new(
            "https://cache.nixos.org/nar/1w1fff338fvdw53sqgamddn1b2xgds473pv6y13gizdbqjv4i5p3.nar.xz",
        )
        .unwrap();
        let nar_file = "1w1fff338fvdw53sqgamddn1b2xgds473pv6y13gizdbqjv4i5p3.nar.xz".to_string();
        let store_path = "/nix/store/zj64jfhbxbync50az13gxr6k7bnqhcb3-codex-0.144.5".to_string();

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
            source.clone(),
            meta.clone(),
        );

        let attrs = NarTransferAttrs {
            nar_file: nar_file.clone(),
            store_path: Some(store_path.clone()),
            substituter_url: meta.url().clone(),
            source_url: source,
            content_length: Some(5),
        };

        let mut tracked = instrument_stream(&metric, attrs, data);
        assert_eq!(metric.active_count(), 1);
        assert_eq!(
            metric.active()[0].store_path.as_deref(),
            Some(store_path.as_str())
        );
        assert_eq!(metric.active()[0].nar_file, nar_file);

        assert_eq!(tracked.inner.next().await.unwrap().unwrap().as_ref(), b"ab");
        assert_eq!(metric.active()[0].bytes_transferred, 2);

        assert_eq!(
            tracked.inner.next().await.unwrap().unwrap().as_ref(),
            b"cde"
        );
        assert_eq!(metric.active()[0].bytes_transferred, 5);
        assert!(tracked.inner.next().await.is_none());

        drop(tracked);
        assert!(metric.is_empty());
    }
}
