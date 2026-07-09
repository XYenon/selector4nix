use std::sync::Arc;

use anyhow::Result as AnyhowResult;
use async_trait::async_trait;
use http::{Method, StatusCode, header};
use selector4nix_streaming::{StreamingClient, StreamingResponse};
use tokio::task::JoinSet;

use crate::domain::common::passthrough_headers::PassthroughHeaders;
use crate::domain::common::url::Url;
use crate::domain::nar_file::model::NarFileLocation;
use crate::domain::nar_file::port::{NarStreamData, NarStreamHeaders, NarStreamProvider};
use crate::infrastructure::config::AppCredential;

pub struct ReqwestNarStreamProvider {
    client: Arc<StreamingClient>,
    credentials: Arc<AppCredential>,
}

impl ReqwestNarStreamProvider {
    pub fn new(client: Arc<StreamingClient>, credentials: Arc<AppCredential>) -> Self {
        Self {
            client,
            credentials,
        }
    }

    fn wrap_ok_response(
        url: Url,
        response: StreamingResponse,
    ) -> AnyhowResult<Option<NarStreamData>> {
        let headers = NarStreamHeaders {
            content_length: response.content_length(),
            content_type: response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(ToString::to_string),
            content_encoding: response
                .headers()
                .get(header::CONTENT_ENCODING)
                .and_then(|v| v.to_str().ok())
                .map(ToString::to_string),
        };

        let stream = response.into_stream();
        Ok(Some(NarStreamData::new(headers, Box::pin(stream), url)))
    }
}

#[async_trait]
impl NarStreamProvider for ReqwestNarStreamProvider {
    async fn stream_nar(
        &self,
        locations: &[NarFileLocation],
        headers: &PassthroughHeaders,
    ) -> AnyhowResult<Option<NarStreamData>> {
        if locations.is_empty() {
            return Ok(None);
        }

        let mut set = JoinSet::new();
        for location in locations {
            let location = location.clone();
            let headers = headers.clone();

            let client = Arc::clone(&self.client);
            let credentials = Arc::clone(&self.credentials);

            set.spawn(async move {
                let request = client
                    .request(Method::GET, location.source_url().value())
                    .configure(|request| {
                        let mut request = request.headers(headers.to_headers());

                        if let Some(credential) = credentials.lookup(location.source_url()) {
                            request = request
                                .basic_auth(credential.login.clone(), credential.secret.clone());
                        }

                        request
                    });

                let response = if let Some(timeout) = location.timeout() {
                    tokio::time::timeout(timeout, request.send()).await
                } else {
                    Ok(request.send().await)
                };
                (location.clone(), response)
            });
        }

        let mut not_found_count = 0;

        while let Some(result) = set.join_next().await {
            let Ok((location, response)) = result else {
                continue;
            };
            let url = location.source_url();

            match response {
                Ok(Ok(response)) => match response.status() {
                    StatusCode::OK => {
                        return Self::wrap_ok_response(url.clone(), response);
                    }
                    StatusCode::NOT_FOUND | StatusCode::FORBIDDEN => {
                        not_found_count += 1;
                    }
                    status => {
                        tracing::debug!(%url, %status, "received unexpected status from substituter");
                    }
                },
                Ok(Err(e)) => {
                    tracing::debug!(%url, error = %e, "failed to request nar from substituter");
                }
                Err(_) => {
                    if let Some(timeout) = location.timeout() {
                        tracing::debug!(%url, timeout_secs = %timeout.as_secs(), "timeout for requesting nar from substituter elapsed");
                    }
                }
            }
        }

        if not_found_count == locations.len() {
            Ok(None)
        } else {
            Err(anyhow::anyhow!("could not fetch nar from any substituter"))
        }
    }
}
