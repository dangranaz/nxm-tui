//! MCP wire types — JSON-RPC 2.0 envelopes + the `sampling` data types.
//!
//! Hand-rolled to honour the project's no-new-dependency constraint: only
//! `serde`/`serde_json` are used. Shapes follow the MCP specification
//! `2025-06-18` (`client/sampling` and `basic/lifecycle`). Field names are the
//! wire names (`camelCase`) mapped via `#[serde(rename_all = "camelCase")]`.
//!
//! Only the subset needed for the minimal sampling host is modelled. Unknown
//! fields are ignored on the wire so future server additions do not break us.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The JSON-RPC protocol version string every message carries.
pub const JSONRPC_VERSION: &str = "2.0";

/// MCP protocol revision this client advertises during `initialize`.
pub const MCP_PROTOCOL_VERSION: &str = "2025-06-18";

/// A JSON-RPC request id. MCP uses numbers or strings; we keep both.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    /// Numeric id (what this client emits).
    Num(i64),
    /// String id (accepted from the server side).
    Str(String),
}

/// A JSON-RPC 2.0 request (has an `id`; expects a matching response).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub jsonrpc: String,
    pub id: RequestId,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl Request {
    /// Build a client→server request with a numeric id.
    pub fn new(id: i64, method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: RequestId::Num(id),
            method: method.into(),
            params,
        }
    }
}

/// A JSON-RPC 2.0 notification (no `id`; no response expected).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl Notification {
    /// Build a client→server notification.
    pub fn new(method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: method.into(),
            params,
        }
    }
}

/// A JSON-RPC 2.0 response — carries either `result` or `error`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub jsonrpc: String,
    pub id: RequestId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl Response {
    /// Build a successful response to a server→client request.
    pub fn ok(id: RequestId, result: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Build an error response to a server→client request.
    pub fn err(id: RequestId, error: RpcError) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

/// A JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl RpcError {
    /// Construct an error with a code and message (no extra data).
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }
}

// ---------------------------------------------------------------------------
// sampling/createMessage — server→client request params & result.
// ---------------------------------------------------------------------------

/// Params of a `sampling/createMessage` request (server→client).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMessageParams {
    /// Conversation the server wants the client's model to continue.
    pub messages: Vec<SamplingMessage>,
    /// Optional system prompt the client SHOULD honour.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// Advisory model-selection preferences (hints + priorities).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_preferences: Option<ModelPreferences>,
    /// Upper bound on tokens to generate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Sampling temperature, if the server requests one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

/// A single message in a sampling conversation. `role` is `user` or
/// `assistant`; `content` is a typed content block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingMessage {
    pub role: String,
    pub content: Content,
}

/// A typed content block. Only `text` is handled by the minimal bridge; image
/// and audio are modelled so deserialization of a mixed conversation does not
/// fail, but they are surfaced as a placeholder to the text backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Content {
    /// UTF-8 text content.
    Text { text: String },
    /// Base64 image content with a MIME type.
    Image {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
    /// Base64 audio content with a MIME type.
    Audio {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
}

impl Content {
    /// A plain text content block.
    pub fn text(t: impl Into<String>) -> Self {
        Content::Text { text: t.into() }
    }

    /// Best-effort text projection used when bridging to a text-only backend.
    pub fn as_text(&self) -> String {
        match self {
            Content::Text { text } => text.clone(),
            Content::Image { mime_type, .. } => format!("[image {mime_type}]"),
            Content::Audio { mime_type, .. } => format!("[audio {mime_type}]"),
        }
    }
}

/// Advisory model-selection preferences (all optional).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelPreferences {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hints: Vec<ModelHint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_priority: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_priority: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intelligence_priority: Option<f32>,
}

/// A model hint — an advisory substring the client MAY map to its own models.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelHint {
    pub name: String,
}

/// Result of a `sampling/createMessage` request (client→server).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMessageResult {
    /// Always `assistant` for a generation.
    pub role: String,
    /// The generated content block.
    pub content: Content,
    /// The concrete model the client used.
    pub model: String,
    /// Why generation stopped (`endTurn`, `maxTokens`, ...).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
}

impl CreateMessageResult {
    /// Build an assistant text result.
    pub fn assistant_text(
        text: impl Into<String>,
        model: impl Into<String>,
        stop_reason: impl Into<String>,
    ) -> Self {
        Self {
            role: "assistant".to_string(),
            content: Content::text(text),
            model: model.into(),
            stop_reason: Some(stop_reason.into()),
        }
    }
}
