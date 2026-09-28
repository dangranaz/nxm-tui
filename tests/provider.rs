//! Provider registry + `/provider` command parsing tests.
//!
//! Compiles the same production sources via `#[path]` (matching
//! `tests/roundtrip.rs`), so no `lib.rs`/`pub` refactor is needed. Covers:
//! - `provider::presets` official URLs/ports (regression guard for the old
//!   Ollama :11435 bug — it must be :11434 now),
//! - `Provider::custom` URL validation and cloud inference,
//! - `all_providers` / `find_provider` merge + lookup,
//! - `app::parse_command` for `/provider list|add|use|remove`.

#![allow(dead_code, unused_imports)]


use nxm_tui::app::{parse_command, Command, ProviderCommand};
use nxm_tui::provider::{all_providers, find_provider, Provider};

#[test]
fn presets_use_official_ports() {
    let presets = Provider::presets();
    let ollama = presets.iter().find(|p| p.name == "Ollama").expect("ollama preset");
    // Regression: Ollama's official OpenAI-compatible port is 11434, not 11435.
    assert_eq!(ollama.base_url, "http://127.0.0.1:11434/v1");
    assert!(!ollama.requires_api_key);

    let lm = presets.iter().find(|p| p.name == "LM Studio").expect("lm studio preset");
    assert_eq!(lm.base_url, "http://127.0.0.1:1234/v1");

    let nvidia = presets.iter().find(|p| p.name == "NVIDIA").expect("nvidia preset");
    assert_eq!(nvidia.base_url, "https://integrate.api.nvidia.com/v1");
    assert!(nvidia.requires_api_key);
    assert!(nvidia.is_cloud);
    assert_eq!(nvidia.api_key_env.as_deref(), Some("NVIDIA_API_KEY"));

    // Nexum Inferentia stays selectable (user decision).
    assert!(presets.iter().any(|p| p.name == "Nexum Inferentia"));
}

#[test]
fn custom_rejects_bad_scheme() {
    assert!(Provider::custom("Foo", "ftp://x").is_err());
    assert!(Provider::custom("", "http://localhost:1").is_err());
    assert!(Provider::custom("Foo", "http://localhost:9000/v1").is_ok());
}

#[test]
fn custom_infers_cloud_and_key_requirement() {
    let local = Provider::custom("Local", "http://127.0.0.1:9000/v1").unwrap();
    assert!(!local.is_cloud);
    assert!(!local.requires_api_key);

    let cloud = Provider::custom("Remote", "https://api.example.com/v1").unwrap();
    assert!(cloud.is_cloud);
    assert!(cloud.requires_api_key);
}

#[test]
fn all_providers_merges_and_overrides() {
    // A custom provider with a preset name overrides the preset.
    let custom = vec![Provider::custom("Ollama", "http://127.0.0.1:9999/v1").unwrap()];
    let merged = all_providers(&custom);
    let ollama = merged.iter().find(|p| p.name == "Ollama").unwrap();
    assert_eq!(ollama.base_url, "http://127.0.0.1:9999/v1");

    // A brand-new custom provider is appended.
    let custom2 = vec![Provider::custom("MyBox", "http://10.0.0.5:8080/v1").unwrap()];
    let merged2 = all_providers(&custom2);
    assert!(merged2.iter().any(|p| p.name == "MyBox"));
    assert_eq!(merged2.len(), Provider::presets().len() + 1);
}

#[test]
fn find_provider_is_case_insensitive() {
    assert!(find_provider("ollama", &[]).is_some());
    assert!(find_provider("NVIDIA", &[]).is_some());
    assert!(find_provider("does-not-exist", &[]).is_none());
}

#[test]
fn parse_provider_list() {
    assert_eq!(parse_command("/provider"), Command::Provider(ProviderCommand::List));
    assert_eq!(parse_command("/provider list"), Command::Provider(ProviderCommand::List));
    assert_eq!(parse_command("/p ls"), Command::Provider(ProviderCommand::List));
}

#[test]
fn parse_provider_add() {
    assert_eq!(
        parse_command("/provider add MyBox http://10.0.0.5:8080/v1"),
        Command::Provider(ProviderCommand::Add {
            name: "MyBox".into(),
            base_url: "http://10.0.0.5:8080/v1".into(),
        })
    );
    // Missing url falls back to Normal (not a valid add).
    assert!(matches!(parse_command("/provider add MyBox"), Command::Normal(_)));
}

#[test]
fn parse_provider_use_and_remove() {
    assert_eq!(
        parse_command("/provider use Ollama"),
        Command::Provider(ProviderCommand::Use("Ollama".into()))
    );
    assert_eq!(
        parse_command("/provider remove MyBox"),
        Command::Provider(ProviderCommand::Remove("MyBox".into()))
    );
    assert_eq!(
        parse_command("/prov rm MyBox"),
        Command::Provider(ProviderCommand::Remove("MyBox".into()))
    );
}
