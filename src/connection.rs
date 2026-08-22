//! Server connection — streams chat responses from any OpenAI-compatible endpoint.

use futures::StreamExt;
use reqwest::Client;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::app::Message;

pub async fn chat_stream(
    client: &Client,
    base_url: &str,
    messages: &[Message],
    tx: mpsc::UnboundedSender<String>,
) -> Result<(), String> {
    let api_messages: Vec<Value> = messages
        .iter()
        .map(|m| {
            let role = match m.role {
                crate::app::Role::User => "user",
                crate::app::Role::Assistant => "assistant",
                crate::app::Role::System => "system",
            };
            serde_json::json!({ "role": role, "content": m.content })
        })
        .collect();

    let body = serde_json::json!({
        "model": "default",
        "messages": api_messages,
        "stream": true,
    });

    // Try /api/chat (Ollama/Nexum Inferentia format) first, then /v1/chat/completions (OpenAI)
    let response = {
        let r = client.post(format!("{base_url}/api/chat")).json(&body).send().await;
        match r {
            Ok(resp) if resp.status().is_success() => resp,
            _ => {
                // Fallback to OpenAI format
                client.post(format!("{base_url}/v1/chat/completions"))
                    .json(&body).send().await
                    .map_err(|e| format!("request failed: {e}"))?
            }
        }
    };

    if !response.status().is_success() {
        return Err(format!("server returned {}", response.status()));
    }

    let mut stream = response.bytes_stream();
    let mut buf = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("stream error: {e}"))?;
        buf.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(line_end) = buf.find('\n') {
            let line = buf[..line_end].trim().to_string();
            buf = buf[line_end + 1..].to_string();

            if line.is_empty() || line.starts_with(": keep-alive") { continue; }

            let json_str = line.strip_prefix("data: ").unwrap_or(&line);
            if json_str == "[DONE]" { return Ok(()); }

            if let Ok(json) = serde_json::from_str::<Value>(json_str) {
                if json.get("done").and_then(|v| v.as_bool()).unwrap_or(false) {
                    return Ok(());
                }
                // OpenAI format: check finish_reason
                if let Some(reason) = json.pointer("/choices/0/finish_reason").and_then(|v| v.as_str()) {
                    if reason == "stop" || reason == "length" {
                        return Ok(());
                    }
                }
                // Ollama/nexum format
                if let Some(content) = json.pointer("/message/content").and_then(|v| v.as_str()) {
                    if !content.is_empty() && tx.send(content.to_string()).is_err() { return Ok(()); }
                }
                // OpenAI format
                if let Some(content) = json.pointer("/choices/0/delta/content").and_then(|v| v.as_str()) {
                    // Filter out <think> tags
                    if content == "<think>" || content == "</think>" { continue; }
                    let trimmed = content.trim();
                    if trimmed.is_empty() { continue; }
                    if tx.send(content.to_string()).is_err() { return Ok(()); }
                }
            }
        }
    }
    Ok(())
}
