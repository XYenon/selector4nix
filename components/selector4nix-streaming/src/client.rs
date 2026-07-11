use std::num::NonZeroUsize;
use std::sync::Arc;

use anyhow::Result as AnyhowResult;
use bytes::Bytes;
use futures::StreamExt;
use http::{HeaderMap, StatusCode, header};
use reqwest::{
    Client, ClientBuilder, Error as ReqwestError, IntoUrl, RequestBuilder, Response, Url,
};
use snafu::{OptionExt, ResultExt, Snafu};

use crate::SBoxStream;
use crate::stream::{
    BoundedHttpStream, ChunkedStream, ChunkedStreamArgs, FullStream, HttpChunkConnector,
};
use crate::throttler::{PerHostHttpThrottler, ThrottlerAdapter, ThrottlerPermit};

const CHUNK_MAX_LEN: NonZeroUsize = NonZeroUsize::new(4 * 1024 * 1024).unwrap();
const WINDOW_MAX_LEN: NonZeroUsize = NonZeroUsize::new(8).unwrap();

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

    pub fn get<U>(&self, url: U) -> StreamingRequest
    where
        U: IntoUrl,
    {
        let url = url.into_url().expect("should be a valid URL");
        let host = url
            .host_str()
            .expect("`url` should have a host")
            .to_string();

        let request = self.client.get(url);
        StreamingRequest {
            request,
            host,
            client: self.client.clone(),
            configure: None,
            throttler: Arc::clone(&self.throttler),
        }
    }
}

pub struct StreamingRequest {
    request: RequestBuilder,
    host: String,
    client: Client,
    configure: Option<Box<dyn Fn(RequestBuilder) -> RequestBuilder + Send + Sync + 'static>>,
    throttler: Arc<PerHostHttpThrottler>,
}

impl StreamingRequest {
    pub fn configure<F>(mut self, func: F) -> Self
    where
        F: Fn(RequestBuilder) -> RequestBuilder + Send + Sync + 'static,
    {
        self.configure = if let Some(prev) = self.configure.take() {
            Some(Box::new(move |request| func(prev(request))))
        } else {
            Some(Box::new(func))
        };
        self
    }

    pub async fn send(self) -> Result<StreamingResponse, StreamHttpBodyError> {
        let permit = self.throttler.acquire(&self.host).await;

        // The first request always asks for the leading chunk so that servers supporting range
        // requests can be served via a chunked response, while servers that ignore `Range` fall
        // back to a full stream.
        let request = (self.configure.as_deref().unwrap_or(&|x| x))(self.request);
        let request = request.header(
            header::RANGE,
            format!("bytes=0-{}", usize::from(CHUNK_MAX_LEN) - 1),
        );
        let response = request.send().await.context(TransportSnafu)?;

        StreamingResponse::from_response(
            response,
            self.client,
            self.configure,
            self.throttler,
            self.host,
            permit,
        )
    }
}

pub enum StreamingResponse {
    Full {
        response: Response,
        content_length: Option<u64>,
        _permit: ThrottlerPermit,
    },
    Chunked {
        response: Response,
        bytes_total: usize,
        initial_chunk_len: usize,
        url: Url,
        host: String,
        client: Client,
        configure: Option<Box<dyn Fn(RequestBuilder) -> RequestBuilder + Send + Sync + 'static>>,
        throttler: Arc<PerHostHttpThrottler>,
        _permit: ThrottlerPermit,
    },
}

impl StreamingResponse {
    fn from_response(
        response: Response,
        client: Client,
        configure: Option<Box<dyn Fn(RequestBuilder) -> RequestBuilder + Send + Sync + 'static>>,
        throttler: Arc<PerHostHttpThrottler>,
        host: String,
        permit: ThrottlerPermit,
    ) -> Result<StreamingResponse, StreamHttpBodyError> {
        match response.status() {
            StatusCode::OK => {
                tracing::debug!(url = %response.url(), "select full (unchunked) stream");
                Ok(StreamingResponse::Full {
                    content_length: response.content_length(),
                    response,
                    _permit: permit,
                })
            }
            StatusCode::PARTIAL_CONTENT => {
                let bytes_total = response
                    .headers()
                    .get(header::CONTENT_RANGE)
                    .and_then(|h| h.to_str().ok())
                    .and_then(|h| h.rsplit_once('/'))
                    .and_then(|(_, total)| total.parse::<usize>().ok())
                    .context(InvalidResponseSnafu {
                        message:
                            "206 Partial Content response is missing a valid Content-Range header",
                    })?;

                let initial_chunk_len = response.content_length().map(|len| len as usize).context(
                    InvalidResponseSnafu {
                        message: "206 Partial Content response is missing Content-Length",
                    },
                )?;

                let url = response.url().clone();
                tracing::debug!(%url, ?bytes_total, ?initial_chunk_len, "select chunked stream");
                Ok(StreamingResponse::Chunked {
                    response,
                    bytes_total,
                    initial_chunk_len,
                    url,
                    host,
                    client,
                    configure,
                    throttler,
                    _permit: permit,
                })
            }
            StatusCode::NOT_FOUND | StatusCode::FORBIDDEN => Err(StreamHttpBodyError::NotFound),
            status => Err(StreamHttpBodyError::InvalidStatus { status }),
        }
    }

    pub fn content_length(&self) -> Option<u64> {
        match self {
            StreamingResponse::Full { content_length, .. } => *content_length,
            StreamingResponse::Chunked { bytes_total, .. } => Some(*bytes_total as u64),
        }
    }

    pub fn raw_headers(&self) -> &HeaderMap {
        match self {
            StreamingResponse::Full { response, .. } => response.headers(),
            StreamingResponse::Chunked { response, .. } => response.headers(),
        }
    }

    pub fn into_stream(self) -> SBoxStream<AnyhowResult<Bytes>> {
        match self {
            StreamingResponse::Full {
                response, _permit, ..
            } => Box::pin(FullStream::new(
                response.bytes_stream().map(|chunk| {
                    anyhow::Context::with_context(chunk, || "failed to read byte stream")
                }),
                _permit,
            )),
            StreamingResponse::Chunked {
                response,
                bytes_total,
                initial_chunk_len,
                client,
                url,
                configure,
                throttler,
                host,
                _permit,
            } => Box::pin(ChunkedStream::new(ChunkedStreamArgs {
                chunk_max_len: CHUNK_MAX_LEN,
                bytes_total,
                window_max_len: WINDOW_MAX_LEN,
                connector: Box::new(HttpChunkConnector::new(client, url, configure)),
                throttler: Box::new(ThrottlerAdapter::new(throttler, host)),
                initial_permit: _permit,
                initial_chunk_stream: Box::pin(BoundedHttpStream::new(
                    Box::pin(response.bytes_stream()),
                    initial_chunk_len,
                )),
            })),
        }
    }
}

#[derive(Snafu, Debug)]
pub enum StreamHttpBodyError {
    #[snafu(display("HTTP transport error"))]
    Transport { source: ReqwestError },
    #[snafu(display("resource not found"))]
    NotFound,
    #[snafu(display("unexpected HTTP status {status}"))]
    InvalidStatus { status: StatusCode },
    #[snafu(display("invalid response from the server: {message}"))]
    InvalidResponse { message: &'static str },
}
