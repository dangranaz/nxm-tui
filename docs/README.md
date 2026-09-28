# nxm-tui-docs (private)

Design, planning, and reference documents for improving **nxm-tui** — the
pure-Rust terminal client (ratatui + crossterm) for OpenAI-compatible LLM
servers.

> These are **copies** gathered from across the `nexum` monorepo on
> 2026-09-08. The originals remain in their source repos; this repo is a
> single private place to work on the TUI's direction. Keep it private — it
> holds internal decisions and roadmap material.

## Layout

| Folder | What it holds | Source |
|--------|---------------|--------|
| `wayfinder/` | The active Wayfinder map for the pure-Rust TUI harness: `MAP.md`, decision/prototype tickets, vendor research, and the `p1-skeleton` prototype. **The primary planning artifact.** | `nxm-harness/.wayfinder/` |
| `handoffs/` | Session handoff: the decision to rebuild nxm-tui from the grok-build base. | `nxm-projects/handoffs/active/` |
| `patterns/` | Vendor/TUI-pattern analyses tagged `tui-patterns` (jcode, oh-my-pi, ACP protocol, vendor analysis). | `nxm-docs/` |
| `history/` | Older plans and brainstorms kept as reference (Inferium v1 / Nexum v2 era). Filenames encode their original path (`__` = `/`). | `nxm-history/` |

## Key orientation

The Wayfinder `MAP.md` destination: *a lightweight, fast, pure-Rust TUI for the
Nexum inference engines* — with `nxm-tui` as the anchor/reuse point. Start
there (`wayfinder/MAP.md` → `wayfinder/tickets/index.md`).

## Related repos

- [`dangranaz/nxm-tui`](https://github.com/dangranaz/nxm-tui) — the TUI client (public)
- [`dangranaz/nxm-tools`](https://github.com/dangranaz/nxm-tools) — tool actuators (public)
