---
id: tui-terminal-ai-analysis
title: "Terminal-AI TUI Analysis — Porting to Ratatui"
status: in-progress
created: 2026-07-15
updated: 2026-07-15T14:30:00Z
linked_to:
  - nexum-terminal
used_for:
  - tui-enhancement
  - ui-planning
links:
  - /vendor/terminal-ai/opencode/packages/tui/
repository: N/A
---

# Terminal-AI TUI Analysis — Porting to Ratatui

> Analisi dettagliata delle funzionalità TUI di OpenCode (terminal-ai) per l'implementazione in nexum-terminal con Ratatui.

## 1. Architettura TUI

### OpenCode (Terminal-AI)
- **Framework**: OpenTUI con SolidJS (React-like)
- **Renderer**: `createCliRenderer` con `render(() => ...)` 
- **State management**: SolidJS Signals (`createSignal`, `createMemo`, `createEffect`)
- **Event system**: Key bindings centralizzate, event bus
- **Plugin system**: Slot-based con `pluginRuntime.Slot`

### Nexum-Terminal (Attuale)
- **Framework**: Ratatui (Rust TUI)
- **Renderer**: `Terminal::draw(|f| render(f, &app))`
- **State management**: Struct mutabile con aggiornamenti manuali
- **Event system**: Handler centralizzato con match su eventi
- **Plugin system**: Non esiste (da implementare)

## 2. Componenti da Portare

### 2.1 Layout Principale (Session Route)

| OpenCode | Nexum-Terminal | Note |
|----------|---------------|------|
| Sidebar (42w) | ❌ Non esiste | Mostra title, workspace, slot plugin |
| Chat history (scrollbox) | ✅ Esiste (render_history) | Manca streaming, markdown pieno |
| Prompt input | ✅ Esiste (render_input) | Manca autocomplete, history |
| Bottom bar (3 righe) | ✅ Esiste (render_bottom_3) | Model role info presente |

### 2.2 Prompt Input Features

| Feature | OpenCode | Nexum | Priorità |
|---------|----------|-------|----------|
| Textarea con multiline | ✅ Solid `TextareaRenderable` | ✅ Text input singola riga | ALTA |
| Autocomplete per filepaths | ✅ `@autocomplete/files.ts` | ❌ | ALTA |
| Autocomplete per comandi | ✅ `@autocomplete/commands.ts` | ❌ | ALTA |
| Prompt history (up/down) | ✅ Hook con persistenza | ❌ | ALTA |
| Editor selection integration | ✅ `editorSelectionKey` | ❌ | MEDIA |
| Paste detection/conversion | ✅ `expandTrackedPastedText` | ❌ | MEDIA |

### 2.3 Session Management

| Feature | OpenCode | Nexum | Priorità |
|---------|----------|-------|----------|
| Session list dialog | ✅ `DialogSessionList` | ✅ `render_sessions_overlay` | BASSA |
| Session rename | ✅ Dialog | ❌ | BASSA |
| Session share (URL) | ✅ `session.share` command | ❌ | BASSA |
| Session timeline/fork | ✅ `DialogTimeline` | ❌ | BASSA |
| Session compact | ✅ `session.compact` | ❌ | BASSA |

### 2.4 Tool Integration

| Feature | OpenCode | Nexum | Priorità |
|---------|----------|-------|----------|
| Tool calling UI | ✅ Part rendering con icone | ❌ | ALTA |
| Tool status spinner | ✅ `Spinner` component | ✅ Braille spinner | MEDIA |
| File read/write visual | ✅ Dialog con path/editor | ❌ | MEDIA |
| Terminal output streaming | ✅ `scrollbox` live | ❌ | ALTA |

## 3. Pattern di Implementazione

### 3.1 State Management (Solid → Rust)

**OpenCode pattern:**
```tsx
const [sidebar, setSidebar] = kv.signal("sidebar", "auto");
const [conceal, setConceal] = createSignal(true);
const session = createMemo(() => sync.session.get(route.sessionID));
```

**Rust equivalente:**
```rust
// Usare struct App con campi opachi
pub struct App {
    pub sidebar_open: bool,
    pub conceal_enabled: bool,
    // ...
}

// Oppure signal-like con Rc<RefCell<>>
use std::rc::Rc;
use std::cell::RefCell;
```

### 3.2 Autocomplete (File/Dir)

**OpenCode:**
```tsx
// Usa @opentui/autocomplete con triggor '/' o path chars
const autocomplete = <Autocomplete triggers={["/", "./", "../"]} />
```

**Rust:**
```rust
// Implementare in handler.rs:
// - Detection di '/' o './'
// - File system scan async (tokio::spawn)
// - Popup overlay con scrollbox
// - Filtering in tempo reale
```

### 3.3 Plugin Slot System

**OpenCode:**
```tsx
<pluginRuntime.Slot name="sidebar_content" session_id={props.sessionID} />
```

**Rust concept:**
```rust
// Trait per plugin TUI
trait TuiPlugin {
    fn render_sidebar(&self, session_id: &str, area: Rect, buf: &mut Buffer);
    fn handle_keypress(&self, key: KeyEvent) -> Option<EventResponse>;
}

// Registro globale in AppState
type PluginSlot = Box<dyn TuiPlugin>;
```

## 4. Feature Gap Analysis

### Must-Have (per coerenza minima)
1. **Sidebar con workspace info** - Mostra directory corrente, git branch
2. **Multiline input** - Supporto per prompt lunghi (Shift+Enter per newline)
3. **Streaming response** - Caratteri che arrivano man mano
4. **Tool part rendering** - Visualizzare i tool calls in chat
5. **Copy-to-clipboard** - Output selection e copy

### Nice-to-Have (enhancement)
1. **Command palette** - Ctrl+P per comandi rapidi
2. **Diff viewer** - Visualizzare file modificati
3. **Session timeline** - Navigare history sessioni
4. **Theme switcher** - Ctrl+T per cambiare tema
5. **Animation toggle** - /toggle animations

## 5. Milestone di Implementazione

| Milestone | Feature | File | Stime |
|-----------|---------|------|-------|
| M1 | Multiline prompt + Shift+Enter | `handler.rs`, `ui.rs` | 2h |
| M2 | Sidebar con workspace/git | `ui.rs` (nuovo modulo sidebar.rs) | 3h |
| M3 | File autocomplete overlay | `autocomplete.rs`, `handler.rs` | 4h |
| M4 | Tool part rendering | `markdown.rs`, `ui.rs` | 3h |
| M5 | Streaming response buffer | `connection.rs`, `handler.rs` | 2h |
| M6 | Session rename/compact | `handler.rs`, `ui.rs` | 2h |
| M7 | Copy selection handler | `handler.rs` | 2h |
| M8 | Plugin slot system (base) | `app.rs`, nuovo plugin.rs | 4h |

## 6. File da Creare/Modificare

### Nuovi file
- `ui/sidebar.rs` - Sidebar rendering
- `ui/autocomplete.rs` - Autocomplete popup
- `ui/tool_part.rs` - Tool part widget
- `plugin.rs` - Plugin trait + registry

### File modificati
- `app.rs` - Aggiungere stato sidebar, autocomplete, tool parts
- `ui.rs` - Integrazione sidebar nella render
- `handler.rs` - Key handling per autocomplete + multiline
- `connection.rs` - Streaming support

## 7. Note Tecnico

### Performance
- OpenCode usa Electron-like renderer (OpenTUI usa GPU acceleration)
- Ratatui è pure CPU, più veloce per terminal
- Evitare ri-render completi: usare `widgets::List` con viewport

### Memory
- OpenCode tiene tutti i message in sync (effetto reattivo)
- Nexum deve limitare history per non saturare RAM
- Implementare virt scrolling per history lunghe

### Testing
- OpenCode ha 100+ test E2E per TUI
- Nexum deve aggiungere test snapshot per render (scomparsimi a mano)