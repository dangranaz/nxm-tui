# Changelog

# 2026-09-07

## feat: configurable LLM providers via /provider commands
- New `src/provider.rs`: `Provider` struct + presets (Nexum Inferentia, Ollama,
  LM Studio, NVIDIA) and custom-provider validation.
- New slash commands: `/provider list|add <name> <url>|use <name>|remove <name>`
  (aliases `/prov`, `/p`).
- Custom providers persisted in `~/.nexum/tui.toml`; API keys stay env-only
  (never on disk).
- NoServer menu now generated dynamically from the provider registry.
- Fix: Ollama preset uses its official OpenAI-compatible port `11434` (was `11435`).
- NVIDIA cloud endpoint (`https://integrate.api.nvidia.com/v1`) with Bearer auth.
- Tests: `tests/provider.rs` (8 tests) — presets, validation, merge/lookup, parsing.

# 2026-08-22

## feat: migrate nxm-tui to GitHub
- Migrated from monorepo (Projects_Tmp/nxm/tui/)
- Converted workspace deps to standalone versions
- 20 source files (app, connection, ui, markdown, history, etc.)
- Added RULES.md, CONTEXT.md, CHANGELOG.md
