//! Model listing and parsing for OpenAI-compatible `/v1/models` endpoints.
//!
//! Supports both OpenAI `{data:[...]}` and Ollama native `{models:[...]}`
//! envelopes, with tolerant parsing per R2 research rules.

use anyhow::{Context, Result};
use serde_json::Value;
use tracing::{info, warn};

/// A single model entry from a `/v1/models` response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelEntry {
    /// Model identifier (e.g. `"llama3.1"`).
    pub id: String,
    /// Owner/organization (e.g. `"library"`, `"meta"`).
    pub owned_by: String,
    /// Creation timestamp (Unix epoch seconds, or 0 if unknown).
    pub created: i64,
}

/// Parse a `/v1/models` JSON response into a list of model entries.
///
/// Accepts both OpenAI `{data:[...]}` and Ollama native `{models:[...]}`
/// envelopes. Tolerant per R2 rules: unknown keys are ignored, missing
/// scalars get defaults, `created` accepts int or ISO string.
///
/// # Examples
///
/// ```rust
/// use nxm_tui::models::parse_models;
/// use serde_json::json;
///
/// let json = json!({
///     "object": "list",
///     "data": [
///         {"id": "llama3.1", "created": 1700000000, "owned_by": "meta"},
///         {"id": "mistral", "created": "2024-01-01T00:00:00Z", "owned_by": "mistralai"}
///     ]
/// });
/// let models = parse_models(&json);
/// assert_eq!(models.len(), 2);
/// assert_eq!(models[0].id, "llama3.1");
/// assert_eq!(models[1].owned_by, "mistralai");
/// ```
pub fn parse_models(json: &Value) -> Vec<ModelEntry> {
    let data = json
        .get("data")
        .and_then(|v| v.as_array())
        .or_else(|| json.get("models").and_then(|v| v.as_array()))
        .cloned()
        .unwrap_or_default();

    data.iter()
        .filter_map(parse_model_entry)
        .take(100) // display cap per R2 rule 5
        .collect()
}

/// Parse a single model entry, returning `None` for empty/invalid entries.
fn parse_model_entry(entry: &Value) -> Option<ModelEntry> {
    let id = entry
        .get("id")
        .and_then(|v| v.as_str())
        .or_else(|| entry.get("name").and_then(|v| v.as_str()))
        .or_else(|| entry.get("model").and_then(|v| v.as_str()))
        .map(|s| s.to_string())?;

    if id.is_empty() {
        return None;
    }

    let owned_by = entry
        .get("owned_by")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let created = parse_created(entry.get("created"));

    Some(ModelEntry {
        id,
        owned_by,
        created,
    })
}

/// Parse `created` field: accepts int (Unix epoch) or ISO 8601 string.
fn parse_created(value: Option<&Value>) -> i64 {
    match value {
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0),
        Some(Value::String(s)) => chrono::DateTime::parse_from_rfc3339(s)
            .map(|dt| dt.timestamp())
            .unwrap_or(0),
        _ => 0,
    }
}

/// Build candidate paths for `/v1/models` fetch, normalizing the base URL.
///
/// Strips a trailing `/v1` (Ollama/LM Studio/NVIDIA already include it) so the
/// first candidate is never `.../v1/v1/models`. Returns three candidates:
/// OpenAI dialect, bare host, and Ollama native.
///
/// # Examples
///
/// ```rust
/// use nxm_tui::models::models_candidate_paths;
///
/// // Base without /v1 — standard case
/// let paths = models_candidate_paths("http://localhost:11434");
/// assert_eq!(paths[0], "http://localhost:11434/v1/models");
/// assert_eq!(paths[1], "http://localhost:11434/models");
/// assert_eq!(paths[2], "http://localhost:11434/api/tags");
///
/// // Base already ending in /v1 — must NOT duplicate
/// let paths = models_candidate_paths("http://localhost:11434/v1");
/// assert_eq!(paths[0], "http://localhost:11434/v1/models");
/// assert_eq!(paths[1], "http://localhost:11434/models");
///
/// // Trailing slash is trimmed
/// let paths = models_candidate_paths("http://localhost:11434/");
/// assert_eq!(paths[0], "http://localhost:11434/v1/models");
/// ```
pub fn models_candidate_paths(base_url: &str) -> Vec<String> {
    let base = base_url.trim_end_matches('/');
    let root = base.strip_suffix("/v1").unwrap_or(base);
    vec![
        format!("{root}/v1/models"), // OpenAI dialect
        format!("{root}/models"),    // bare host
        format!("{root}/api/tags"),  // Ollama native
    ]
}

/// Fetch models from an OpenAI-compatible `/v1/models` endpoint.
///
/// Tries `{base}/v1/models` first, then falls back to `{base}/models` and
/// Ollama native `{base}/api/tags` on 404. Returns a user-friendly error
/// message for common failure modes (server down, auth required).
///
/// # Examples
///
/// ```rust,no_run
/// use nxm_tui::models::fetch_models;
///
/// # async fn example() {
/// let client = reqwest::Client::new();
/// match fetch_models(&client, "http://localhost:11434/v1", None).await {
///     Ok(models) => println!("Found {} models", models.len()),
///     Err(e) => eprintln!("Error: {e}"),
/// }
/// # }
/// ```
pub async fn fetch_models(
    client: &reqwest::Client,
    base_url: &str,
    api_key: Option<&str>,
) -> Result<Vec<ModelEntry>> {
    let paths = models_candidate_paths(base_url);

    let mut last_err = None;

    for path in &paths {
        match fetch_models_path(client, path, api_key).await {
            Ok(models) => {
                info!(target: "nexum::models", count = models.len(), path = %path, "models fetched");
                return Ok(models);
            }
            Err(e) => {
                warn!(target: "nexum::models", path = %path, error = %e, "fetch failed");
                last_err = Some(e);
            }
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("no models endpoint found")))
}

/// Fetch models from a single path.
async fn fetch_models_path(
    client: &reqwest::Client,
    path: &str,
    api_key: Option<&str>,
) -> Result<Vec<ModelEntry>> {
    let mut req = client.get(path);
    if let Some(key) = api_key {
        if !key.is_empty() {
            req = req.header("Authorization", format!("Bearer {key}"));
        }
    }

    let resp = req.send().await.map_err(|e| {
        if e.is_timeout() || e.is_connect() {
            anyhow::anyhow!("server down: {e}")
        } else {
            anyhow::anyhow!("request failed: {e}")
        }
    })?;

    let status = resp.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(anyhow::anyhow!(
            "authentication required — use `/provider set-key <name>` to add an API key"
        ));
    }

    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(anyhow::anyhow!("not found (404)"));
    }

    if !status.is_success() {
        return Err(anyhow::anyhow!("server returned {status}"));
    }

    let json: Value = resp.json().await.context("invalid JSON response")?;
    Ok(parse_models(&json))
}
