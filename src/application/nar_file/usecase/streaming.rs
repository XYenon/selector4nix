use std::sync::Arc;

use crate::application::nar_file::active_download::{ActiveDownloadRegistry, track_stream};
use crate::application::nar_file::actor::{NarFileActorRegistry, NarFileRequest};
use crate::domain::common::passthrough_headers::PassthroughHeaders;
use crate::domain::nar_file::model::NarFileKey;
use crate::domain::nar_file::port::NarStreamData;
use crate::domain::nar_info::NarInfoRepository;
use crate::domain::nar_info::model::StorePathHash;
use crate::{AppError, AppResultExt};

pub struct NarFileStreamingUseCase {
    nar_file_registry: Arc<NarFileActorRegistry>,
    nar_info_repository: Arc<dyn NarInfoRepository>,
    active_downloads: Arc<ActiveDownloadRegistry>,
}

impl NarFileStreamingUseCase {
    pub fn new(
        nar_file_registry: Arc<NarFileActorRegistry>,
        nar_info_repository: Arc<dyn NarInfoRepository>,
        active_downloads: Arc<ActiveDownloadRegistry>,
    ) -> Self {
        Self {
            nar_file_registry,
            nar_info_repository,
            active_downloads,
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

        let data = response
            .inspect(|data| {
                tracing::info!(
                    nar_file = %key.to_file_name().value(),
                    source_url = %data.source_url,
                    substituter = %data.substituter.url(),
                    "streamed nar from substituter"
                );
            })
            .inspect_err(|err| {
                tracing::warn!(nar_file = %key.to_file_name().value(), %err, "failed to stream nar");
            })?;

        let store_path = self.lookup_store_path(data.store_path_hash.as_ref()).await;
        Ok(track_stream(
            &self.active_downloads,
            &key,
            data,
            store_path.as_deref(),
        ))
    }

    async fn lookup_store_path(&self, hash: Option<&StorePathHash>) -> Option<String> {
        let hash = hash?;
        let nar_info = self.nar_info_repository.get(hash).await.ok().flatten()?;
        nar_info
            .nar_info()
            .and_then(|data| data.store_path().map(str::to_string))
    }
}
