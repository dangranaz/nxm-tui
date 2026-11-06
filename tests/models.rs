//! Tests for the models module: parser (two envelopes, fallbacks, edge cases)
//! and command parsing for /models, /model use, /provider remove-key.

use nxm_tui::app::{parse_command, Command, ProviderCommand};
use nxm_tui::models::{models_candidate_paths, parse_models};
use serde_json::json;

// ── Parser: OpenAI envelope ──────────────────────────────────────────────────

#[test]
fn parse_openai_envelope() {
    let json = json!({
        "object": "list",
        "data": [
            {"id": "llama3.1", "created": 1700000000, "owned_by": "meta"},
            {"id": "mistral", "created": 1700000001, "owned_by": "mistralai"}
        ]
    });
    let models = parse_models(&json);
    assert_eq!(models.len(), 2);
    assert_eq!(models[0].id, "llama3.1");
    assert_eq!(models[0].owned_by, "meta");
    assert_eq!(models[0].created, 1700000000);
    assert_eq!(models[1].id, "mistral");
    assert_eq!(models[1].owned_by, "mistralai");
}

// ── Parser: Ollama native envelope ───────────────────────────────────────────

#[test]
fn parse_ollama_envelope() {
    let json = json!({
        "models": [
            {"name": "llama3.1:latest", "model": "llama3.1:latest"},
            {"name": "mistral:latest", "model": "mistral:latest"}
        ]
    });
    let models = parse_models(&json);
    assert_eq!(models.len(), 2);
    assert_eq!(models[0].id, "llama3.1:latest");
    assert_eq!(models[0].owned_by, "unknown"); // default
    assert_eq!(models[0].created, 0); // default
    assert_eq!(models[1].id, "mistral:latest");
}

// ── Parser: id fallback chain ────────────────────────────────────────────────

#[test]
fn parse_id_fallback_name_then_model() {
    // Entry with only "name"
    let json = json!({"data": [{"name": "model-a"}]});
    let models = parse_models(&json);
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].id, "model-a");

    // Entry with only "model"
    let json = json!({"data": [{"model": "model-b"}]});
    let models = parse_models(&json);
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].id, "model-b");

    // Entry with both "id" and "name" — "id" wins
    let json = json!({"data": [{"id": "primary", "name": "secondary"}]});
    let models = parse_models(&json);
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].id, "primary");
}

// ── Parser: created int vs ISO string ────────────────────────────────────────

#[test]
fn parse_created_int_and_iso_string() {
    let json = json!({"data": [
        {"id": "a", "created": 1700000000},
        {"id": "b", "created": "2024-01-15T10:30:00Z"},
        {"id": "c"} // missing created → default 0
    ]});
    let models = parse_models(&json);
    assert_eq!(models.len(), 3);
    assert_eq!(models[0].created, 1700000000);
    assert_eq!(models[1].created, 1705314600); // 2024-01-15T10:30:00Z
    assert_eq!(models[2].created, 0);
}

// ── Parser: empty entries skipped ────────────────────────────────────────────

#[test]
fn parse_empty_entries_skipped() {
    let json = json!({"data": [
        {"id": "valid"},
        {"id": ""},           // empty id → skipped
        {"name": ""},         // empty name → skipped
        {},                   // no id/name/model → skipped
        {"id": "also-valid"}
    ]});
    let models = parse_models(&json);
    assert_eq!(models.len(), 2);
    assert_eq!(models[0].id, "valid");
    assert_eq!(models[1].id, "also-valid");
}

// ── Parser: unknown keys ignored ─────────────────────────────────────────────

#[test]
fn parse_unknown_keys_ignored() {
    let json = json!({"data": [
        {
            "id": "model-x",
            "owned_by": "org",
            "created": 1700000000,
            "extra_field": "ignored",
            "nested": {"foo": "bar"},
            "permission": [{"id": "perm-1"}]
        }
    ]});
    let models = parse_models(&json);
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].id, "model-x");
    assert_eq!(models[0].owned_by, "org");
}

// ── Parser: empty list ───────────────────────────────────────────────────────

#[test]
fn parse_empty_list() {
    let json = json!({"data": []});
    let models = parse_models(&json);
    assert!(models.is_empty());

    let json = json!({"models": []});
    let models = parse_models(&json);
    assert!(models.is_empty());

    let json = json!({});
    let models = parse_models(&json);
    assert!(models.is_empty());
}

// ── Parser: display cap (100) ────────────────────────────────────────────────

#[test]
fn parse_display_cap() {
    let data: Vec<_> = (0..150)
        .map(|i| json!({"id": format!("model-{i}")}))
        .collect();
    let json = json!({"data": data});
    let models = parse_models(&json);
    assert_eq!(models.len(), 100);
}

// ── Command parsing: /models ─────────────────────────────────────────────────

#[test]
fn parse_command_models() {
    assert_eq!(parse_command("/models"), Command::Models);
}

// ── Command parsing: /model use ──────────────────────────────────────────────

#[test]
fn parse_command_model_use() {
    assert_eq!(
        parse_command("/model use 1"),
        Command::ModelUse("1".to_string())
    );
    assert_eq!(
        parse_command("/model use llama3.1"),
        Command::ModelUse("llama3.1".to_string())
    );
    // Missing arg → Normal
    assert_eq!(
        parse_command("/model use"),
        Command::Normal("/model use".to_string())
    );
}

// ── Command parsing: /provider remove-key ────────────────────────────────────

#[test]
fn parse_command_provider_remove_key() {
    assert_eq!(
        parse_command("/provider remove-key NVIDIA"),
        Command::Provider(ProviderCommand::RemoveKey("NVIDIA".to_string()))
    );
    assert_eq!(
        parse_command("/provider rm-key Ollama"),
        Command::Provider(ProviderCommand::RemoveKey("Ollama".to_string()))
    );
    // Missing arg → Normal
    assert_eq!(
        parse_command("/provider remove-key"),
        Command::Normal("/provider remove-key".to_string())
    );
}

// ── Command parsing: existing commands still work ────────────────────────────

#[test]
fn parse_command_existing_still_work() {
    assert_eq!(parse_command("/help"), Command::Help);
    assert_eq!(parse_command("/clear"), Command::Clear);
    assert_eq!(parse_command("/quit"), Command::Quit);
    assert_eq!(
        parse_command("/provider list"),
        Command::Provider(ProviderCommand::List)
    );
    assert_eq!(
        parse_command("/provider set-key NVIDIA"),
        Command::Provider(ProviderCommand::SetKey("NVIDIA".to_string()))
    );
}

// ── models_candidate_paths: /v1 normalization ─────────────────────────────────

#[test]
fn candidate_paths_without_v1_suffix() {
    let paths = models_candidate_paths("http://localhost:11434");
    assert_eq!(paths[0], "http://localhost:11434/v1/models");
    assert_eq!(paths[1], "http://localhost:11434/models");
    assert_eq!(paths[2], "http://localhost:11434/api/tags");
}

#[test]
fn candidate_paths_with_v1_suffix_not_duplicated() {
    // Base already ends in /v1 — must NOT produce .../v1/v1/models
    let paths = models_candidate_paths("http://localhost:11434/v1");
    assert_eq!(paths[0], "http://localhost:11434/v1/models");
    assert_eq!(paths[1], "http://localhost:11434/models");
    assert_eq!(paths[2], "http://localhost:11434/api/tags");
    // Explicit guard: no path contains the double /v1/v1
    assert!(!paths.iter().any(|p| p.contains("/v1/v1")));
}

#[test]
fn candidate_paths_with_trailing_slash() {
    let paths = models_candidate_paths("http://localhost:11434/");
    assert_eq!(paths[0], "http://localhost:11434/v1/models");
    assert_eq!(paths[1], "http://localhost:11434/models");
}

#[test]
fn candidate_paths_with_v1_and_trailing_slash() {
    let paths = models_candidate_paths("http://localhost:11434/v1/");
    assert_eq!(paths[0], "http://localhost:11434/v1/models");
    assert!(!paths.iter().any(|p| p.contains("/v1/v1")));
}
