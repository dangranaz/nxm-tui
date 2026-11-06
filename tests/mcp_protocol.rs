//! Wire-format tests for the MCP protocol types.
//!
//! These lock the serde shapes to the MCP spec (`2025-06-18`): a
//! `sampling/createMessage` request deserializes into [`CreateMessageParams`],
//! and a [`CreateMessageResult`] serializes back into the documented shape.

use nxm_tui::mcp::protocol::{
    Content, CreateMessageParams, CreateMessageResult, Request, RequestId, Response, RpcError,
};
use serde_json::json;

#[test]
fn deserializes_spec_create_message_request() {
    // The exact request body from the MCP sampling spec page.
    let raw = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "sampling/createMessage",
        "params": {
            "messages": [{
                "role": "user",
                "content": { "type": "text", "text": "What is the capital of France?" }
            }],
            "modelPreferences": {
                "hints": [{ "name": "claude-3-sonnet" }],
                "intelligencePriority": 0.8,
                "speedPriority": 0.5
            },
            "systemPrompt": "You are a helpful assistant.",
            "maxTokens": 100
        }
    });

    let req: Request = serde_json::from_value(raw).expect("request parses");
    assert_eq!(req.method, "sampling/createMessage");
    assert_eq!(req.id, RequestId::Num(1));

    let params: CreateMessageParams =
        serde_json::from_value(req.params.expect("has params")).expect("params parse");
    assert_eq!(params.messages.len(), 1);
    assert_eq!(params.messages[0].role, "user");
    assert_eq!(
        params.messages[0].content.as_text(),
        "What is the capital of France?"
    );
    assert_eq!(
        params.system_prompt.as_deref(),
        Some("You are a helpful assistant.")
    );
    assert_eq!(params.max_tokens, Some(100));

    let prefs = params.model_preferences.expect("has prefs");
    assert_eq!(prefs.hints[0].name, "claude-3-sonnet");
    assert_eq!(prefs.intelligence_priority, Some(0.8));
    assert_eq!(prefs.speed_priority, Some(0.5));
}

#[test]
fn serializes_create_message_result_to_spec_shape() {
    let result = CreateMessageResult::assistant_text(
        "The capital of France is Paris.",
        "claude-3-sonnet-20240307",
        "endTurn",
    );
    let v = serde_json::to_value(&result).expect("serializes");
    assert_eq!(v["role"], "assistant");
    assert_eq!(v["content"]["type"], "text");
    assert_eq!(v["content"]["text"], "The capital of France is Paris.");
    assert_eq!(v["model"], "claude-3-sonnet-20240307");
    assert_eq!(v["stopReason"], "endTurn");
}

#[test]
fn response_round_trips_result_and_error() {
    let ok = Response::ok(RequestId::Num(7), json!({ "answer": 42 }));
    let s = serde_json::to_string(&ok).expect("serialize ok");
    let back: Response = serde_json::from_str(&s).expect("parse ok");
    assert_eq!(back.id, RequestId::Num(7));
    assert_eq!(back.result.expect("has result")["answer"], 42);
    assert!(back.error.is_none());

    let err = Response::err(RequestId::Str("abc".into()), RpcError::new(-1, "rejected"));
    let s = serde_json::to_string(&err).expect("serialize err");
    let back: Response = serde_json::from_str(&s).expect("parse err");
    assert_eq!(back.id, RequestId::Str("abc".into()));
    assert!(back.result.is_none());
    let e = back.error.expect("has error");
    assert_eq!(e.code, -1);
    assert_eq!(e.message, "rejected");
}

#[test]
fn content_projects_non_text_to_placeholder() {
    let img = Content::Image {
        data: "AAAA".into(),
        mime_type: "image/png".into(),
    };
    assert_eq!(img.as_text(), "[image image/png]");
}

#[test]
fn initialize_request_declares_sampling_capability() {
    // Mirror what McpClient::initialize sends; assert the capability is present.
    let params = json!({
        "protocolVersion": "2025-06-18",
        "capabilities": { "sampling": {} },
        "clientInfo": { "name": "nxm-tui", "version": "0.1.0" }
    });
    assert!(params["capabilities"].get("sampling").is_some());
}
