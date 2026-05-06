use std::sync::Arc;

use selector4nix_actor::actor::AnyAddress;

use crate::application::{AppErrorKind, AppOptionExt, AppResult, AppResultExt};
use crate::domain::nar::index::{NarFileEvent, NarFileIndex};
use crate::domain::nar::model::NarFileName;
use crate::domain::nar::port::{NarStreamData, NarStreamProvider};
use crate::domain::substituter::index::SubstituterAvailabilityIndex;
use crate::domain::substituter::model::Url;

pub struct NarStreamingUseCase {
    substituter_availability_index: Arc<dyn SubstituterAvailabilityIndex>,
    nar_stream_provider: Arc<dyn NarStreamProvider>,
    nar_file_index: Arc<dyn NarFileIndex>,
    nar_file_index_pub: AnyAddress<NarFileEvent>,
}

impl NarStreamingUseCase {
    pub fn new(
        substituter_availability_index: Arc<dyn SubstituterAvailabilityIndex>,
        nar_stream_provider: Arc<dyn NarStreamProvider>,
        nar_file_index: Arc<dyn NarFileIndex>,
        nar_file_index_pub: AnyAddress<NarFileEvent>,
    ) -> Self {
        Self {
            substituter_availability_index,
            nar_stream_provider,
            nar_file_index,
            nar_file_index_pub,
        }
    }

    pub async fn stream_nar(&self, nar_file: &NarFileName) -> AppResult<NarStreamData> {
        tracing::info!(nar_file = %nar_file.value(), "acquiring nar stream from substituter");

        if let Some(source_url) = &self.nar_file_index.get_source_url(nar_file).await {
            tracing::info!(nar_file = %nar_file.value(), source_url = %source_url, "use cached nar file location");

            let urls = [source_url.clone()];
            let outcome = self.nar_stream_provider.stream_nar(&urls).await;

            if let Ok(Some(data)) = outcome {
                return Ok(data);
            } else {
                tracing::warn!(nar_file = %nar_file.value(), "fallback to query all substituters for nar file location")
            }
        } else {
            tracing::info!(nar_file = %nar_file.value(), "query all substituters for nar file location");
        }

        self.stream_nar_from_all(nar_file).await
    }

    async fn stream_nar_from_all(&self, nar_file: &NarFileName) -> AppResult<NarStreamData> {
        let urls = self.build_fallback_urls(nar_file);
        let outcome = self.nar_stream_provider.stream_nar(&urls).await;

        match &outcome {
            Ok(Some(NarStreamData { source_url, .. })) => {
                tracing::info!(nar_file = %nar_file.value(), %source_url, "streamed nar from substituter");
                let request = NarFileEvent::Registered {
                    nar_file: nar_file.clone(),
                    source_url: source_url.clone(),
                };
                let _ = self.nar_file_index_pub.tell(request).await;
            }
            Ok(None) => {
                tracing::info!(nar_file = %nar_file.value(), "failed to find nar file in any substituter");
            }
            Err(_) => {
                tracing::warn!(nar_file = %nar_file.value(), "failed to stream nar");
            }
        }

        outcome.wrap(AppErrorKind::Infrastructure).flat()
    }

    fn build_fallback_urls(&self, nar_file: &NarFileName) -> Vec<Url> {
        self.substituter_availability_index
            .query_all()
            .iter()
            .map(|substituter| {
                let prefix = substituter.target().storage_url();
                nar_file.with_storage_prefix(prefix)
            })
            .collect()
    }
}
