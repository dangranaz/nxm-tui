---
id: vendor-oh-my-pi-001
title: "oh-my-pi — AI Coding Agent for the Terminal"
status: planned
created: 2026-06-06
updated: 2026-06-06T14:30:00Z
linked_to: []
used_for:
  - vendor
  - coding-agent
  - rust-core
  - terminal
links:
  - https://github.com/can1357/oh-my-pi
  - https://omp.sh
repository: https://github.com/can1357/oh-my-pi
---

# oh-my-pi — AI Coding Agent for the Terminal

## 1. Identity

| Field | Value |
|-------|-------|
| **Vendor** | `can1357/oh-my-pi` |
| **Role** | Reference — terminal coding agent with Rust core (~27k lines) |
| **License** | MIT |
| **Stars** | ~7,800 |
| **Language** | TypeScript (Bun) + Rust |
| **Submodule** | `vendor/oh-my-pi/` |

## 2. What It Provides

Terminal-native AI coding agent with deep IDE integration:

| Component | Purpose | Nexum Relevance |
|-----------|---------|-----------------|
| Rust Native Engine | Shell (brush), grep, AST parsing, structural summarization | ⚡ Rust patterns for native performance |
| Hash-anchored edits | Prevent concurrent agent file conflicts | Reference for multi-agent coordination |
| LSP/DAP integration | Symbol-aware editing, real debugging | Architecture reference |
| Subagent orchestration | Fan-out work across isolated workers | Multi-agent patterns |
| 40+ AI providers | Unified streaming abstraction | Provider integration patterns |

### Key Architecture

```
Entry Points (TUI, Node SDK, RPC, ACP)
  → Agent Core (sessions, tool calling, state)
    → Tool Surface (Files, Runtime, Code Intelligence, Coordination)
      → Rust Native Engine (27k lines: shell, grep, AST, summarization)
```

## 3. Why Vendor (Submodule)

oh-my-pi's Rust core provides direct architectural reference for:
- **Native tool performance**: How to implement high-performance code intelligence in Rust
- **Agent session management**: Session persistence, branching, context compaction
- **Provider abstraction**: Unified interface across 40+ LLM providers
- **Hash-anchored editing**: Content-hash based file edits for concurrency safety

## 4. Integration Plan

- Study Rust core patterns for potential adoption in nexum-srv tool layer
- Reference session management for nexum's multi-agent future
- Hash-anchored edit algorithm for concurrent file operations

## 5. Upgrade Strategy

- Track upstream releases via `git submodule update --remote`
- Monitor Rust core changes that may affect patterns we adopt
- Evaluate new features quarterly for relevance to nexum
