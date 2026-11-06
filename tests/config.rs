//! TuiConfig round-trip tests (loopeng-01 R4): `max_steps` survives
//! serialize→deserialize via toml, and a toml written before loopeng-01
//! (no `max_steps` key) loads as `None` thanks to `#[serde(default)]`.

use nxm_tui::config::TuiConfig;

#[test]
fn max_steps_survives_toml_roundtrip() {
    let cfg = TuiConfig {
        max_steps: Some(7),
        ..TuiConfig::default()
    };
    let s = toml::to_string_pretty(&cfg).unwrap();
    let back: TuiConfig = toml::from_str(&s).unwrap();
    assert_eq!(back.max_steps, Some(7));
}

#[test]
fn missing_max_steps_key_loads_none() {
    // A pre-loopeng-01 tui.toml has no `max_steps` key: it must still load,
    // with `max_steps` falling back to `None` (default applied at the call site).
    let legacy = r#"
endpoint = "http://localhost:8080"
model_name = "test-model"
sidebar_open = true
max_context = 8192
"#;
    let cfg: TuiConfig = toml::from_str(legacy).unwrap();
    assert_eq!(cfg.max_steps, None);
    assert_eq!(cfg.endpoint, Some("http://localhost:8080".to_string()));
}

#[test]
fn stall_threshold_survives_toml_roundtrip() {
    let cfg = TuiConfig {
        stall_threshold: Some(5),
        ..TuiConfig::default()
    };
    let s = toml::to_string_pretty(&cfg).unwrap();
    let back: TuiConfig = toml::from_str(&s).unwrap();
    assert_eq!(back.stall_threshold, Some(5));
}

#[test]
fn missing_stall_threshold_key_loads_none() {
    // A pre-loopeng-01-02 tui.toml has no `stall_threshold` key: it must
    // still load, with `stall_threshold` falling back to `None` (default
    // applied at the call site).
    let legacy = r#"
endpoint = "http://localhost:8080"
model_name = "test-model"
"#;
    let cfg: TuiConfig = toml::from_str(legacy).unwrap();
    assert_eq!(cfg.stall_threshold, None);
    assert_eq!(cfg.endpoint, Some("http://localhost:8080".to_string()));
}
