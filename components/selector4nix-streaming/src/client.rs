use std::pin::Pin;
use std::sync::Arc;

use anyhow::{Context as _, Result as AnyhowResult};
use bytes::Bytes;
use futures::{Stream, StreamExt};
use http::{HeaderMap, Method, StatusCode};
use reqwest::{Client, ClientBuilder, Error as ReqwestError, IntoUrl, RequestBuilder, Response};

use crate::stream::FullStream;
use crate::throttler::{PerHostHttpThrottler, ThrottlerPermit};

pub struct StreamingClient {
    client: Client,
    throttler: Arc<PerHostHttpThrottler>,
}

impl StreamingClient {
    pub fn new(client: ClientBuilder, max_concurrent_requests: usize) -> Self {
        Self {
            client: client
                .http1_only()
                .build()
                .expect("invalid reqwest client configuration"),
            throttler: Arc::new(PerHostHttpThrottler::new(max_concurrent_requests)),
        }
    }

    pub fn request<U>(&self, method: Method, url: U) -> StreamingRequest
    where
        U: IntoUrl,
    {
        let url = url.into_url().expect("should be a valid URL");
        let host = url
            .host_str()
            .expect("`url` should have a host")
            .to_string();

        let request = self.client.request(method, url);
        StreamingRequest {
            inner: request,
            host,
            throttler: Arc::clone(&self.throttler),
        }
    }
}

pub struct StreamingRequest {
    inner: RequestBuilder,
    host: String,
    throttler: Arc<PerHostHttpThrottler>,
}

impl StreamingRequest {
    pub fn configure<F>(mut self, func: F) -> Self
    where
        F: FnOnce(RequestBuilder) -> RequestBuilder,
    {
        self.inner = func(self.inner);
        self
    }

    pub async fn send(self) -> Result<StreamingResponse, ReqwestError> {
        let permit = self.throttler.acquire(&self.host).await;
        let response = self.inner.send().await?;
        Ok(StreamingResponse {
            inner: response,
            _permit: permit,
        })
    }
}

pub struct StreamingResponse {
    inner: Response,
    _permit: ThrottlerPermit,
}

impl StreamingResponse {
    pub fn content_length(&self) -> Option<u64> {
        self.inner.content_length()
    }

    pub fn status(&self) -> StatusCode {
        self.inner.status()
    }

    pub fn headers(&self) -> &HeaderMap {
        self.inner.headers()
    }

    pub fn into_stream(self) -> Pin<Box<dyn Stream<Item = AnyhowResult<Bytes>> + Send>> {
        Box::pin(FullStream::new(
            self.inner
                .bytes_stream()
                .map(|chunk| chunk.with_context(|| "failed to read nar stream")),
            self._permit,
        ))
    }
}
