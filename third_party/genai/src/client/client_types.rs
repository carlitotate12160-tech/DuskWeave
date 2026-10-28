// Modified for DuskWeave: bounded non-streaming HTTP I/O and per-call evidence.
use crate::webc::WebClient;
use crate::{ClientBuilder, ClientConfig, Result};
use std::sync::Arc;

/// Client for sending AI requests to supported providers.
///
/// Construct with:
/// - [`ClientBuilder::default()`] followed by `.build()`, or
///
/// - [`Client::builder()`], which is equivalent to `ClientBuilder::default()`.
#[derive(Debug, Clone)]
pub struct Client {
	pub(super) inner: Arc<ClientInner>,
}

// region:    --- Client Constructors

impl Client {
	/// Returns a builder for configuring and constructing a [`Client`].
	///
	/// Equivalent to calling [`ClientBuilder::default()`].
	pub fn builder() -> ClientBuilder {
		ClientBuilder::default()
	}

	pub fn new() -> Result<Client> {
		Client::builder().build()
	}
}

// endregion: --- Client Constructors

// region:    --- Client Getters

impl Client {
	pub(crate) fn web_client(&self) -> &WebClient {
		&self.inner.web_client
	}

	pub(crate) fn config(&self) -> &ClientConfig {
		&self.inner.config
	}
}

// endregion: --- Client Getters

// region:    --- ClientInner

#[derive(Debug)]
pub(super) struct ClientInner {
	pub(super) web_client: WebClient,

	pub(super) config: ClientConfig,
}

// endregion: --- ClientInner

/// One non-streaming call's entity observations. Missing measurements remain unavailable.
#[derive(Default)]
pub struct ChatCallEvidence {
	pub request_entity_bytes: Option<usize>,
	pub response_entity_bytes: Option<usize>,
	pub http_status: Option<u16>,
	/// Bounded transient entity data. Never include it in diagnostics or persistence.
	pub raw_response: Option<Vec<u8>>,
}
impl std::fmt::Debug for ChatCallEvidence {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ChatCallEvidence")
			.field("request_entity_bytes", &self.request_entity_bytes)
			.field("response_entity_bytes", &self.response_entity_bytes)
			.field("http_status", &self.http_status)
			.field("raw_response", &self.raw_response.as_ref().map(|_| "[redacted]"))
			.finish()
	}
}

/// The ordinary chat result and observations belong to this invocation only.
pub struct ObservedChatResponse {
	pub result: crate::Result<crate::chat::ChatResponse>,
	pub evidence: ChatCallEvidence,
}
