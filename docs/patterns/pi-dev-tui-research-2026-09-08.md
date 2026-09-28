# Ricerca TUI terminale stile pi.dev — documenti e vendors utili (nxm-memory)

Data: 2026-09-08
Fonte indice: workspaces `nxm` (~/Projects/nexum) + `vendors` (~/Projects/nexum/vendors)
Metodo: `nxm-memory_index_search` + `search_docs` + `search_code` + `get_chunk` (fonti primarie, non write-up secondari)
Destinazione d'uso: nuova TUI a terminale come pi.dev, ma pure-Rust (ratatui + crossterm) — evoluzione di `nxm-tui`.

> Nota Wayfinder esistente: `nxm-harness/.wayfinder/MAP.md` ha già deciso stack
> `ratatui + crossterm + tokio + clap`, relazione con `nxm-tui` = evolve in place,
> TS/Ink out come implementazione ma tenuti come riferimento comportamentale.
> Questa ricerca non ri-decide lo stack, mappa solo cosa riusare.

## 1. Riferimento pi.dev diretto (TypeScript — comportamentale, non implementativo)

### 1.1 `vendors/pi-agent/packages/tui/` — motore di rendering pi-tui
- `vendors/pi-agent/packages/tui/README.md:1`:
  framework minimale con differential rendering, synchronized output (CSI 2026, no flicker),
  bracketed paste, interfaccia `TUI` intercambiabile (main-screen + alt-screen),
  componenti: Text, TruncatedText, Input, Editor, Markdown, Loader, SelectList,
  SettingsList, Spacer, Image, Box, Container, VStack, HStack, ScrollView.
  Inline images Kitty/iTerm2, autocomplete path + slash commands.
- Pattern chiave da portare in Rust: `TuiMainScreen` vs `TuiAltScreen` dietro stessa interfaccia;
  test `test/overlay-non-capturing.test.ts:994,740,340` e `test/tui-alt-screen.test.ts:641,507` —
  overlay focus/non-capturing, VirtualTerminal 80x24 per test senza TTY.
- Da copiare: suite di test con terminale virtuale per overlay/alt-screen in `nxm-tui`.

### 1.2 `vendors/pi-agent/packages/coding-agent/docs/tui.md:101` — contratto integrazione
- Due layer: rendering engine (`packages/tui`) + integration layer (`packages/coding-agent`).
- `ctx.ui.custom()` montato in editor-area o overlay, focused, deve chiamare `done(result)`.
  In background/headless (`hasUI===false`) e RPC mode è no-op — guardare con `ctx.hasUI`.
- Esempio estensione: `vendors/pi-agent/packages/coding-agent/examples/extensions/overlay-qa-tests.ts:1,194`.

### 1.3 `vendors/oh-my-pi/` — fork/documentazione più leggibile dello stesso motore
- `vendors/oh-my-pi/docs/tui.md:1,51` — contratto Component più preciso:
  ```ts
  interface Component { render(width:number): readonly string[]; handleInput?(data:string): void; }
  ```
  Stesso array reference se invariato → memoization container + stable-prefix;
  `Focusable { focused:boolean }`, cursore via `CURSOR_MARKER` (non getCursorPosition);
  vincoli terminal-safety: mai superare `width`, renderer tronca come ultima difesa.
- `vendors/oh-my-pi/packages/tui/README.md:1` — stessa feature list, ottima come checklist
  differenziale per `nxm-tui`: three-strategy diff, CSI 2026, bracketed paste >10 righe.
- `vendors/oh-my-pi/docs/tui.md` + `packages/coding-agent` = spec per custom-tool UI / extension UI.

### 1.4 `vendors/kimi-code/` — implementazione pi-tui più matura + skill di scrittura TUI
- `vendors/kimi-code/packages/pi-tui/README.md:1` — identico a sopra (@moonshot-ai/pi-tui).
- `vendors/kimi-code/apps/kimi-code/AGENTS.md:1` → punta a skill `write-tui`.
- `vendors/kimi-code/.agents/skills/write-tui/SKILL.md:1` — **da leggere per intero prima di toccare la TUI**:
  architettura `KimiTUI` coordinator (`src/tui/kimi-tui.ts`) che delega a controllers:
  `session-event-handler.ts` (dispatch `handleEvent` + `handleXxx`),
  `streaming-ui.ts` (assistant delta, thinking, tool call/result, compaction, subagent),
  `session-replay.ts` + `utils/message-replay.ts` (resume riusa i live render hooks),
  `editor-keyboard.ts`, `auth-flow.ts`, `tasks-browser.ts`;
  `commands/` (declare/parse), `components/chrome|dialogs|editor|media|messages|panes`,
  `reverse-rpc/` (approval/question callback → panel), `theme/` single source reference.
  Regole: mai accumulare logica in `KimiTUI`, split per tipo di feature, DESIGN.md normativo per dialog.
- `vendors/kimi-code/apps/kimi-code/src/tui/components/dialogs/approval-preview.ts:1` —
  pattern nested-takeover per approval diff, riusabile per tool-approval in Rust.

## 2. Riferimenti Rust ratatui (implementativi — riuso diretto)

### 2.1 `nxm-tui/CONTEXT.md:1`, `nxm-tui/README.md:1` — anchor in-house
- Ratatui + crossterm, reqwest SSE streaming, tokio async, markdown rendering,
  session history/persistence, autocomplete comandi, TOML config.
  Multi-pane già esistente (`sidebar/history/prompt/mode_bar/overlays/tool_types` per MAP).
- Decisione D3 confermata in `nxm-harness/.wayfinder/MAP.md:51`: evolve in place,
  agentic REPL piegato dentro `nxm-tui`, P2 parte da `nxm-ai/nxm-tui@main`.

### 2.2 `vendors/codex/codex-rs/tui/` (+ copia `vendors/open-interpreter/codex-rs/tui/`) — miglior riferimento Rust
- `vendors/codex/codex-rs/tui/src/chatwidget.rs:1`:
  `ChatWidget` consuma protocol events, celle `HistoryCell` committed + `active_cell`
  mutabile in-place durante streaming (coalesced exec/tool group);
  transcript overlay (`Ctrl+T`) con cached live tail via `active_cell_transcript_key()`;
  bottom pane singolo indicatore "task running" derivato da `agent_turn_running` + `mcp_startup_status`;
  nasconde status row durante commentary streaming per evitare doppi spinner.
- Altri file da minare: `app/event_dispatch.rs:19`, `bottom_pane/`, `markdown_render/`,
  `model_migration.rs:1`, `resume_picker.rs:1`, `app/tests/patch_approval_tests.rs:134`,
  `permission_shortcuts_tests.rs:1`.
- Pattern streaming: tokio channel → `Frame` redraw, celle storia virtualizzate.

### 2.3 `vendors/openshell/.agents/skills/tui-development/SKILL.md:1,401` — guida ratatui completa
- Crate `crates/openshell-tui/`, `ratatui` (`frame.size()`), `crossterm` backend,
  `tokio` event loop + mpsc, `tonic` TLS gRPC, discovery via `list_gateways()`.
- Gerarchia Gateway > Workspace > Sandbox/Provider/Settings > Logs — esempio di data model gerarchico.
- UX: keybinding contestuali per focus (`[h/l] Switch Tab [j/k] Navigate [Enter] Edit [d] Delete [Esc] Back [q] Quit`),
  Theme adaptive dark/light, fetching periodico via tick.
- Da riusare come template di SKILL.md per `nxm-tui`.

### 2.4 `nxm-docs/vendor/agents/jcode-Tier1.md:1` — jcode Rust agent
- Repo https://github.com/1jehuang/jcode, MIT, 18.7k★, 7000+ commit.
- `handterm` custom renderer, ~28MB RAM, ~14ms startup; multi-provider
  (Claude/OpenAI/Gemini/Copilot/Ollama/vLLM), swarm mode, memoria semantica, MCP client.
- Score ⭐⭐⭐⭐ per TUI patterns `nxm-tui`. Da studiare per startup veloce + routing multi-provider.

### 2.5 `nxm-harness/.wayfinder/research/vendor-interrogation.md:51` — matrice versioni confermata
- `llmfit-tui`: ratatui 0.30 / crossterm 0.29 / tokio 1.52 / clap 4.6 — hardware-aware, lista modelli virtualizzata.
- `openviking ov_cli`: ratatui 0.29 / crossterm 0.28 / tokio 1.38 / clap 4.5.
- `nxm-tui`: ratatui 0.29 / crossterm 0.28 / tokio 1 + reqwest 0.12.
- Decisione: pinna a 0.29/0.28 per consistenza con `nxm-tui` oppure latest 0.30/0.29.

## 3. Riferimenti comportamentali TS (non adottare come implementazione)

- `vendors/hermes-agent/ui-tui/README.md:1` — React + Ink (TS owns screen, Python owns sessions),
  JSON-RPC newline-delimited su stdio (`entry.tsx` ↔ `python -m tui_gateway.entry`),
  `GatewayClient`, protocol_error/stderr ring. Utile solo se si valuta split TS/Python — scartato per Rust puro.
- `vendors/gemini-cli/.gemini/skills/agent-tui/SKILL.md:51,151` — automazione terminale, `wait "text"`.
- `vendors/openhuman/docs/plans/tui-chat-plan.md:1,51` — piano Logs-first 4-tab, alt-screen + raw mode, viewport scrollback PgUp/PgDn/mouse.
- `vendors/kimi-code`, `gemini-cli` confermati TS in `vendor-interrogation.md` R3 — solo behaviour.

## 4. Storia interna (analisi già fatta — non rifare)

- `nxm-history/nexum_v2_old/nexum_docs/plans/tui-terminal-ai-analysis.md:1` —
  analisi OpenCode (OpenTUI + SolidJS signals) → Ratatui (`Terminal::draw`, struct mutabile).
  Tabella porting: Sidebar 42w mancante, chat history senza streaming/markdown pieno,
  prompt senza autocomplete/history, bottom bar 3 righe esistente.
- `nxm-history/nexum_v2_old/nexum_docs/plans/tui-terminal-ai-plan.md:1` — piano stile OpenCode.
- `nxm-history/inferium_v1_old/.../inferium-tui.md:51`, `nxm-history/projects_tmp_archived/tui/README.md:1`,
  `nxm-history/nexum_v2_old/ui/nexum-terminal/README.md:1` — vecchi TUI ratatui, keybindings IT.
- `nxm-tui-docs/patterns/` contiene già: `jcode-Tier1.md`, `oh-my-pi-Tier1.md`,
  `VENDOR-TUI-ANALYSIS.md`, `VENDORS-TUI.md`, `acp-agent-client-protocol.md`.

## 5. Cosa usare subito per la nuova TUI stile pi.dev

1. **Motore**: resta su `ratatui 0.29 + crossterm 0.28 + tokio` (consistenza `nxm-tui`), valuta 0.30/0.29 solo se serve.
2. **Copia da pi-tui (TS) in Rust**: Component `render(width)->lines` + reference-equality memo,
   `CURSOR_MARKER`, CSI 2026 sync output, bracketed paste, overlay focus/non-capturing,
   `TuiMainScreen/TuiAltScreen` dietro stessa interfaccia, VirtualTerminal per test.
3. **Copia da kimi-code**: split `KimiTUI` → controllers (`streaming-ui`, `session-event-handler`,
   `session-replay` che riusa live hooks), `components/messages/tool-renderers/registry.ts` per tool-result,
   `mountEditorReplacement` per dialog, theme single source reference, DESIGN.md normativo.
4. **Copia da codex-rs**: `HistoryCell` + `active_cell` streaming, transcript overlay Ctrl+T con cache key,
   task-running derivato, approval/permission flows, `markdown_render::render_markdown_text_with_width`.
5. **Copia da openshell**: struttura SKILL.md + event loop tokio/mpsc + status bar contestuale + tick refresh.
6. **Non rifare**: rileggi `tui-terminal-ai-analysis.md` + `MAP.md Decisions so far` prima di aprire ticket.

## Fonti (chunk IDs nxm-memory)

- `vendors/oh-my-pi/packages/tui/README.md:1`, `vendors/oh-my-pi/docs/tui.md:1,51`
- `vendors/pi-agent/packages/tui/README.md:1`, `vendors/pi-agent/packages/tui/test/overlay-non-capturing.test.ts:994,740,340`, `tui-alt-screen.test.ts:641,507`, `packages/coding-agent/docs/tui.md:101`, `examples/extensions/overlay-qa-tests.ts:1,194`
- `vendors/kimi-code/packages/pi-tui/README.md:1`, `vendors/kimi-code/.agents/skills/write-tui/SKILL.md:1`, `vendors/kimi-code/apps/kimi-code/AGENTS.md:1`, `apps/kimi-code/src/tui/components/dialogs/approval-preview.ts:1`
- `vendors/codex/codex-rs/tui/src/chatwidget.rs:1`, `app/event_dispatch.rs:22`, `app/tests/patch_approval_tests.rs:134`
- `vendors/openshell/.agents/skills/tui-development/SKILL.md:1,401`
- `vendors/hermes-agent/ui-tui/README.md:1`, `vendors/gemini-cli/.gemini/skills/agent-tui/SKILL.md:51`
- `nxm-tui/CONTEXT.md:1`, `nxm-docs/vendor/agents/jcode-Tier1.md:1`
- `nxm-history/nexum_v2_old/nexum_docs/plans/tui-terminal-ai-analysis.md:1`, `tui-terminal-ai-plan.md:1`
- `nxm-harness/.wayfinder/MAP.md:51`, `nxm-harness/.wayfinder/research/vendor-interrogation.md:51`
