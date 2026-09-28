# Vendor list — utili per nxm-tui

> Sottoinsieme dei vendor (`<workspace>/nexum/vendors`) rilevanti
> per migliorare **nxm-tui** (TUI Rust pura: ratatui 0.29 + crossterm 0.28 +
> tokio + reqwest). Filtrato dalla lista generale `vendors/list_vendors.md`.
> Aggiornato: 2026-09-08. Analisi di dettaglio: [`VENDOR-TUI-ANALYSIS.md`](./VENDOR-TUI-ANALYSIS.md).

## TUI Rust (ratatui) — candidati copia/replica

| Vendor | Path | Linguaggio | ratatui | Licenza | Rilevanza TUI | Verdetto |
|--------|------|-----------|---------|---------|---------------|----------|
| **grok-build** | `vendors/grok-build/crates/codegen/` | Rust | workspace | Apache-2.0 | **Altissima** — markdown streaming+syntect, pager scrollback+search, inline viewport | Copia (solo `search/`) + Replica (resto) |
| **codex** | `vendors/codex/codex-rs/tui/` | Rust | 0.30.2 | Apache-2.0 | **Alta** — streaming render incrementale, diff render, hyperlinks OSC8 | Replica (ratatui 0.30 → API diverse) |
| **open-interpreter** | `vendors/open-interpreter/codex-rs/tui/` | Rust | 0.30 | Apache-2.0 | Media — fork/gemello di codex-rs (stessi moduli) | Replica |
| **llmfit** | `vendors/llmfit/llmfit-tui/` | Rust | std | MIT | Media — lista modelli virtualizzata, hardware-aware (utile per D5-hardware-fit) | Replica (uso idiomatico) |
| **hindsight** | `vendors/hindsight/hindsight-cli/` | Rust | std | MIT | Bassa/media — CLI TUI di memoria | Riferimento |
| **uzu** | `vendors/uzu/` | Rust | std | MIT | Bassa — engine con CLI | Riferimento |
| **openviking** | `vendors/openviking/` (`ov_cli`) | Rust | 0.29 | Apache-2.0 | Media — **ratatui 0.29 come nxm-tui**; `wrap_display_text` unicode-aware copiabile | **Copia** (`wrap_display_text`) |
| **higgs** | `vendors/higgs/` | Rust | std | — | Bassa | Riferimento |
| **openshell** | `vendors/openshell/` | Rust | std | — | Bassa — shell NVIDIA | Riferimento |
| **oxillama** | `vendors/oxillama/` | Rust | std | — | Bassa | Riferimento |

## CLI agentiche (riferimento comportamentale — NON Rust)

Non adottabili come codice (TypeScript/Go), ma utili come riferimento di UX:
pattern di streaming, multi-pane, command palette `/`, markdown.

| Vendor | Linguaggio | Cosa guardare |
|--------|-----------|---------------|
| `deepseek-harness` | TS (pnpm monorepo, MIT) | **architettura "everything is a plugin"**; `packages/{cli,client,terminal}` per pattern harness/UX; 215k★, org ufficiale deepseek-ai |
| `gemini-cli` | TS + ink | streaming strutturato, layout |
| `kimi-code` | TS (pnpm monorepo) | agentic coding CLI, tool loop |
| `opencode` | TS | plan/build mode, sessions |
| `aider` | Python | diff-apply UX |
| `oh-my-pi` | — | terminal coding agent |

## Priorità di adozione (impatto/effort)

Dal report di analisi, ordine consigliato:

0. **COPIA** `openviking wrap_display_text` → wrap unicode-aware, corregge il bug di troncamento/panic UTF-8 del `markdown.rs` attuale (S, indipendente)
1. **COPIA** `grok-build/.../xai-grok-pager/src/search/` → ricerca nello scrollback (S, self-contained, solo `regex`)
2. Virtualizzazione viewport con `List`/`ListState` (rif. `llmfit-tui`) — S–M
3. **Markdown + syntect** (rif. `xai-grok-markdown`) — L, copre il gap #1
4. Streaming render incrementale prefisso-stabile + tail (rif. `codex streaming/`) — M
5. Hyperlink OSC 8 (rif. grok `hyperlinks.rs`, più portabile di codex) — S–M
6. Cache highlight resumable (rif. grok `open_code_highlighter.rs`) — M
7. Diff rendering (rif. `codex diff_render.rs`) — M

## Note licenza

- Tutti i candidati Rust sono **permissivi** (Apache-2.0 o MIT) → copia/derivazione
  consentita **con attribuzione**. Per Apache-2.0 (grok-build, codex, openviking):
  conservare copyright + `NOTICE` se presente (grok/codex non hanno NOTICE separato;
  vale l'attribuzione standard §4). Aggiungere le fonti in un `THIRD-PARTY-NOTICES.md`
  di nxm-tui quando si copia/replica codice.
- Le dipendenze proposte (syntect, two-face, pulldown-cmark, regex, unicode-width,
  diffy/similar, url/linkify) sono MIT/Apache-2.0 → compatibili con la licenza MIT di nxm-tui.
