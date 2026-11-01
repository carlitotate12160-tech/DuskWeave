// Modified for DuskWeave: bounded non-streaming HTTP I/O and per-call evidence.
use crate::webc::{Error, Result};
use crate::{Headers, WebConfig};
use reqwest::header::CONTENT_TYPE;
use reqwest::{Method, RequestBuilder, StatusCode};
use serde_json::Value;

/// A simple reqwest client wrapper for this library.
/// Here are some good default
/// ```rust
/// let reqwest_client = reqwest::Client::builder()
///    .tcp_nodelay(true)
///    .gzip(true)
///    .pool_max_idle_per_host(4)
///    .http2_keep_alive_interval(Some(Duration::from_secs(20)))
///    .http2_keep_alive_timeout(Duration::from_secs(10))
///    .http2_keep_alive_while_idle(true)
///    .http2_adaptive_window(true)
///    .build()
///    .map_err(Error::Reqwest)?;
/// ```
#[derive(Debug)]
pub struct WebClient {
	reqwest_client: reqwest::Client,
}

// region:    --- Constructors

impl WebClient {
	pub fn from_reqwest_client(reqwest_client: reqwest::Client) -> Self {
		WebClient { reqwest_client }
	}
}

// endregion: --- Constructors

// region:    --- Web Method Implementation

impl WebClient {
	pub async fn do_get(&self, url: &str, headers: &Headers) -> Result<WebResponse> {
		let mut reqwest_builder = self.reqwest_client.request(Method::GET, url);

		for (k, v) in headers.iter() {
			reqwest_builder = reqwest_builder.header(k, v);
		}
		let reqwest_res = reqwest_builder.send().await?;

		let response = WebResponse::from_reqwest_response(reqwest_res).await?;

		Ok(response)
	}

	pub async fn do_post(&self, url: &str, headers: &Headers, content: &Value) -> Result<WebResponse> {
		self.do_post_observed(url, headers, content).await.0
	}

	pub(crate) async fn do_post_observed(
		&self,
		url: &str,
		headers: &Headers,
		content: &Value,
	) -> (Result<WebResponse>, crate::ChatCallEvidence) {
		let mut evidence = crate::ChatCallEvidence::default();
		let result = async {
			let builder = self.new_req_builder(url, headers, content)?;
			let request = builder.build()?;
			evidence.request_entity_bytes = request.body().and_then(|b| b.as_bytes()).map(<[u8]>::len);
			let response = self.reqwest_client.execute(request).await?;
			WebResponse::read_observed(response, &mut evidence).await
		}
		.await;
		if let Err(Error::RequestEntityLimit { observed, .. }) = &result {
			evidence.request_entity_bytes = Some(*observed);
		}
		(result, evidence)
	}

	pub fn new_req_builder(&self, url: &str, headers: &Headers, content: &Value) -> Result<RequestBuilder> {
		let method = Method::POST;

		let mut reqwest_builder = self.reqwest_client.request(method, url);
		for (k, v) in headers.iter() {
			reqwest_builder = reqwest_builder.header(k, v);
		}
		let bytes = serde_json::to_vec(content).map_err(|_| Error::RequestSerialization)?;
		if bytes.len() > WebConfig::REQUEST_ENTITY_LIMIT {
			return Err(Error::RequestEntityLimit {
				limit: WebConfig::REQUEST_ENTITY_LIMIT,
				observed: bytes.len(),
			});
		}
		// Count and send the same serialized UTF-8 bytes; never serialize a second time.
		reqwest_builder = reqwest_builder.header(CONTENT_TYPE, "application/json").body(bytes);

		Ok(reqwest_builder)
	}
}
// endregion: --- Web Method Implementation

// region:    --- WebResponse

// NOTE: This is not a non-streaming web response (assumed to be JSON for this library).
//       Streaming is handled with event-source or custom streams (for example, for Cohere).

#[derive(Debug)]
pub struct WebResponse {
	#[allow(unused)]
	pub status: StatusCode,
	pub body: Value,
}

impl WebResponse {
	/// Note 1: For now, assume only a JSON response.
	/// Note 2: Currently, the WebResponse holds a Value (parsed from the entire body), and then the caller
	///         can cherry-pick/deserialize further. In the future, we might consider returning `body: String`
	///         to enable more optimized parsing, allowing for selective parsing constrained by the structure.
	pub(crate) async fn from_reqwest_response(res: reqwest::Response) -> Result<WebResponse> {
		Self::read_observed(res, &mut crate::ChatCallEvidence::default()).await
	}

	async fn read_observed(mut res: reqwest::Response, evidence: &mut crate::ChatCallEvidence) -> Result<WebResponse> {
		let status = res.status();
		evidence.http_status = Some(status.as_u16());
		evidence.response_entity_bytes = Some(0);
		let is_json = res
			.headers()
			.get(CONTENT_TYPE)
			.and_then(|v| v.to_str().ok())
			.is_some_and(|v| v.starts_with("application/json"));
		let mut bytes = Vec::with_capacity(WebConfig::RESPONSE_ENTITY_LIMIT);
		while let Some(chunk) = res.chunk().await? {
			let observed = bytes.len().saturating_add(chunk.len());
			evidence.response_entity_bytes = Some(observed);
			if chunk.len() > WebConfig::RESPONSE_ENTITY_LIMIT - bytes.len() {
				return Err(Error::ResponseEntityLimit {
					status,
					limit: WebConfig::RESPONSE_ENTITY_LIMIT,
					observed,
				});
			}
			bytes.extend_from_slice(&chunk);
		}
		let entity_bytes = bytes.len();
		evidence.raw_response = Some(bytes);
		if !status.is_success() {
			return Err(Error::BoundedStatus { status, entity_bytes });
		}
		if !is_json {
			return Err(Error::BoundedNotJson { status, entity_bytes });
		}
		let body = serde_json::from_slice(evidence.raw_response.as_deref().unwrap_or_default())
			.map_err(|_| Error::BoundedInvalidJson { status, entity_bytes })?;
		Ok(WebResponse { status, body })
	}
}

// endregion: --- WebResponse
