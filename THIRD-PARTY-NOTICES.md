# Third-Party Notices

This project includes code adapted from third-party sources. The original
copyright and licenses are reproduced below.

## grok-build (xai-org/grok-build)

- **Copyright:** Copyright 2023-2026 SpaceXAI
- **License:** Apache License 2.0
- **Vendor snapshot:** commit `2bdd1d6` (aligned with upstream), vendored at
  `vendors/agents/grok-build/`.
- **Usage (search):** `crates/codegen/xai-grok-pager-render/src/search`
  (`mod.rs`, `matcher.rs`) → the text search module (`src/search.rs` —
  `QueryKind`, `TextMatcher`, `next_index_after`, `prev_index_before`) was
  adapted from the above source. The adaptation replaces the compiled-regex
  field with `Option<regex::Regex>` to remove all production
  `unwrap()`/`expect()` fallbacks.
- **Usage (markdown renderer, V3):** `crates/codegen/xai-grok-markdown` and
  `crates/codegen/xai-grok-markdown-core` → the rich markdown renderer under
  `src/md/` is a copy-identical-in-behaviour port (pulldown-cmark pipeline,
  element rendering, styling, streaming). Kept 100% pure-Rust and independent
  of the upstream `xai-*` workspace: `syntect` stays on `default-fancy` (no
  onig/C); where upstream relied on non-Rust or workspace-internal code, the
  equivalent is rewritten in Rust. Large files are split across modules to
  satisfy the repo's 300-line cap; no functionality is reduced. Per-file
  `//! Derived from xai-grok-markdown (Apache-2.0). Modified: ...` headers mark
  each ported file.

A copy of the Apache License 2.0 is reproduced in `LICENSE-APACHE` and available
at <https://www.apache.org/licenses/LICENSE-2.0>.

## syntect (trishume/syntect)

- **Source:** `https://github.com/trishume/syntect`
- **Copyright:** Christopher Brown and contributors
- **License:** MIT License
- **Usage:** `src/highlight.rs` uses `syntect` (v5.3.0, `default-features = false`,
  `features = ["default-fancy"]`) for per-token syntax highlighting of code
  fences. Pure-Rust via `fancy-regex` — no `onig`, no C compiler. Color
  mapping (`syntect::highlighting::Color` → `ratatui::style::Color::Rgb`) is
  implemented locally in `src/highlight.rs`; no `syntect-tui` is used.

A copy of the MIT License is available at
<https://opensource.org/licenses/MIT>.

## openviking (volcengine/OpenViking)

- **Source:** `crates/ov_cli/src/commands/render_utils.rs` (`wrap_display_text`,
  `append_word_to_line`) — the `crates/ov_cli` component only. The repository
  root of OpenViking is AGPL-3.0; no code was taken from it.
- **Vendor snapshot:** commit `f6010a5`, vendored at
  `vendors/tooling/openviking/`.
- **Copyright:** OpenViking Contributors
- **License:** Apache License 2.0 (`crates/ov_cli/Cargo.toml`, `crates/LICENSE`)
- **Usage:** `src/markdown.rs` (`wrap`, `append_word`) is adapted from the above
  source: words are appended while they fit the row's cell width and a word
  wider than the row is split on character boundaries using `unicode_width`.
  The adaptation drops upstream's `max_lines`/ellipsis truncation (the
  transcript renders every line) and renames the helpers; the upstream
  truncation marker `…` is kept for code-fence lines.

A copy of the Apache License 2.0 is available at
<https://www.apache.org/licenses/LICENSE-2.0>.
