use crate::api::api_utils::get_headers_from_request;
use crate::api::model::provider_stream_factory::{create_provider_stream, BufferStreamOptions};
use crate::debug_if_enabled;
use crate::model::config::ConfigInput;
use crate::utils::request_utils::{get_request_headers, mask_sensitive_info};
use actix_web::{HttpRequest};
use bytes::Bytes;
use futures::stream::BoxStream;
use log::error;
use reqwest::StatusCode;
use std::sync::Arc;
use futures::TryStreamExt;
use url::Url;
use crate::api::model::model_utils::get_response_headers;
use crate::api::model::stream_error::StreamError;

type ProviderStreamResponse = (Option<BoxStream<'static, Result<Bytes, StreamError>>>, Option<(Vec<(String, String)>, StatusCode)>);

pub async fn get_provider_pipe_stream(http_client: &Arc<reqwest::Client>,
                                      stream_url: &Url,
                                      req: &HttpRequest,
                                      input: Option<&ConfigInput>) -> ProviderStreamResponse {
    let req_headers = get_headers_from_request(req, &None);
    debug_if_enabled!("Stream requested with headers: {:?}", req_headers.iter().map(|header| (header.0, String::from_utf8_lossy(header.1))).collect::<Vec<_>>());
    // These are the configured headers for this input.
    let input_headers = input.map(|i| i.headers.clone());
    // The stream url, we need to clone it because of move to async block.
    // We merge configured input headers with the headers from the request.
    let headers = get_request_headers(input_headers.as_ref(), Some(&req_headers));
    let client = http_client.get(stream_url.clone()).headers(headers.clone());
    match client.send().await {
        Ok(mut response) => {
            let response_headers = get_response_headers(&mut response);
            let status = response.status();
            if status.is_success() {
                (Some(Box::pin(response.bytes_stream().map_err(|err|StreamError::reqwest(&err)))), Some((response_headers, status)))
            } else {
                (None, Some((response_headers, status)))
            }
        }
        Err(err) => {
            let masked_url = mask_sensitive_info(stream_url.as_str());
            error!("Failed to open stream {masked_url} {err}");
            (None, None)
        }
    }
}

pub async fn get_provider_reconnect_buffered_stream(http_client: &Arc<reqwest::Client>,
                                                    stream_url: &Url,
                                                    req: &HttpRequest,
                                                    input: Option<&ConfigInput>,
                                                    options: BufferStreamOptions) -> ProviderStreamResponse {
    match create_provider_stream(Arc::clone(http_client), stream_url, req, input, options).await {
        None => (None, None),
        Some((stream, info)) => {
            (Some(stream), info)
        }
    }
}
