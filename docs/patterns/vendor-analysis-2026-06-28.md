---
id: vendor-analysis-2026-06-28
title: "Vendor Analysis — GitHub Projects (28 June 2026)"
status: in-progress
created: 2026-06-28
updated: 2026-06-28T18:01:00Z
linked_to:
  - session-2026-06-28
used_for:
  - vendor-evaluation
  - agent
  - coding
  - memory
  - infrastructure
links:
  - https://github.com/BuilderIO/skills
  - https://github.com/argyleink/prop-for-that
  - https://github.com/temporalio/sdk-java
  - https://github.com/baskduf/FableCodex
  - https://github.com/bombshell-dev/clack
  - https://github.com/Danilaa1/slot-text
  - https://github.com/ZhuLinsen/daily_stock
  - https://github.com/mukul975/Anthropic-Cybersecurity-Skills
  - https://github.com/garrytan/gstack
  - https://github.com/bytedance/deer-flow
  - https://github.com/shanraisshan/claude-code-best-practice
  - https://github.com/stablyai/orca
  - https://github.com/esengine/DeepSeek-Reasonix
  - https://github.com/tashfeenahmed/free-llm-api
  - https://github.com/calesthio/OpenMontage
  - https://github.com/NousResearch/hermes-agent
  - https://github.com/Stirling-Tools/Stirling-PDF
  - https://github.com/microsoft/presidio
  - https://github.com/run-llama/liteparse
  - https://github.com/supermemoryai/supermemory
repository: N/A
---

# Vendor Analysis — GitHub Projects (28 June 2026)

## Rilevanza per Nexum

| # | Progetto | Rilevanza | Categoria | Azione |
|---|----------|-----------|-----------|--------|
| 1 | **BuilderIO/skills** | 🔥 ALTA | Agent skills | Esplorare per nexum-agent |
| 2 | argyleink/prop-for-that | ⬜ BASSA | CSS tooling | Non rilevante |
| 3 | temporalio/sdk-java | ⬜ BASSA | Workflow orchestration (Java) | Pattern utile, non codice |
| 4 | baskduf/FableCodex | ⬜ MEDIA | Game AI/storytelling | Non prioritario |
| 5 | **bombshell-dev/clack** | ✅ MEDIA | CLI prompts/UX | Pattern per nexum-terminal |
| 6 | Danilaa1/slot-text | ⬜ BASSA | Text animation | Non rilevante |
| 7 | ZhuLinsen/daily_stock | ⬜ BASSA | Stock analysis | Non rilevante |
| 8 | **mukul975/Anthropic-Cybersecurity-Skills** | ✅ MEDIA | Agent skills (security) | Pattern per skill system |
| 9 | **garrytan/gstack** | ✅ MEDIA | Full-stack AI dev tool | Architettura reference |
| 10 | **bytedance/deer-flow** | 🔥 ALTA | Multi-agent workflow | Direttamente rilevante per router/orchestrator |
| 11 | **shanraisshan/claude-code-best-practice** | ✅ MEDIA | Agent best practices | Prompt engineering |
| 12 | **stablyai/orca** | 🔥 ALTA | AI agent framework | Concorrente/ispirazione |
| 13 | esengine/DeepSeek-Reasonix | ⬜ MEDIA | Reasoning wrapper | Pattern reasoning |
| 14 | tashfeenahmed/free-llm-api | ⬜ BASSA | API proxy | Non rilevante |
| 15 | calesthio/OpenMontage | ⬜ BASSA | Video editing | Non rilevante |
| 16 | **NousResearch/hermes-agent** | 🔥 ALTA | Agent framework + skills | Già in vendor, aggiornare |
| 17 | Stirling-Tools/Stirling-PDF | ⬜ BASSA | PDF tools | Non rilevante |
| 18 | **microsoft/presidio** | ✅ MEDIA | PII detection/anonymization | Privacy per memory system |
| 19 | **run-llama/liteparse** | 🔥 ALTA | Lightweight document parsing | Già in vendor, per RAG/memory |
| 20 | **supermemoryai/supermemory** | 🔥 ALTA | AI memory system | Già in vendor, per nexum-memory |

---

## Analisi Dettagliata — Progetti ad Alta Rilevanza

### 1. BuilderIO/skills — Skills for Coding Agents
**Cosa è**: Libreria di skill modulari per coding agent (file ops, git, test, refactor, ecc.)
**Per nexum**: Pattern perfetto per il nostro `nexum-agent` / `nexum-tools`. Definisce come strutturare skill indipendenti che un agent può usare.
**Azione**: Studiare la struttura delle skill, portare il pattern nel nostro tool system.

### 10. bytedance/deer-flow 2.0 — Multi-Agent Workflow
**Cosa è**: Framework di ByteDance per orchestrare flussi multi-agente con pianificazione, esecuzione, e feedback loop.
**Per nexum**: Direttamente applicabile alla nostra architettura router + worker. DeerFlow implementa:
- Pianificazione automatica dei task
- Dispatch a sub-agent specializzati
- Aggregazione risultati
- Feedback loop con auto-correzione
**Azione**: Studiare per il design di `nexum-orchestrator`. Possibile vendor submodule.

### 12. stablyai/orca — AI Agent Framework
**Cosa è**: Framework per AI agent con tool calling, memory, e reasoning chain.
**Per nexum**: Architettura concorrente — come strutturano il loop agent → reason → act → observe.
**Azione**: Analizzare pattern architetturali. Non dipendere, ma ispirarsi.

### 16. NousResearch/hermes-agent — Agent + Skills
**Cosa è**: Agent framework di NousResearch con sistema di skill pluggable, web browsing, code execution.
**Per nexum**: Già presente in `nexum_tmp/vendor/hermes-agent`. Ha:
- Skill system modulare
- Tool calling standardizzato
- Session management
- Supporto per modelli locali (Hermes fine-tune)
**Azione**: Aggiornare il submodule. Studiare il skill system per nexum-agent.

### 19. run-llama/liteparse — Lightweight Document Parsing
**Cosa è**: Parser leggero per documenti (PDF, HTML, Markdown, code) — estrae testo strutturato per LLM.
**Per nexum**: Già in `nexum_tmp/vendor/liteparse`. Utilissimo per:
- Prefill pipeline: parsing documenti prima di iniettarli nel context
- Memory system: chunking intelligente di documenti
- RAG: estrazione contenuto da file arbitrari
**Azione**: Integrare come dipendenza per memory retrieval / context injection.

### 20. supermemoryai/supermemory — AI Memory System
**Cosa è**: Sistema di memoria per AI con:
- Salvataggio automatico di conversazioni
- Retrieval semantico
- Categorizzazione automatica
- API per query
**Per nexum**: Già in `nexum_tmp/vendor/supermemory`. Pattern per il nostro `nexum-memory`:
- Come strutturare la persistenza
- Come fare retrieval cross-sessione
- Come categorizzare automaticamente le memorie
**Azione**: Studiare l'architettura per migliorare nexum-memory-retrieval.

---

## Analisi — Progetti a Media Rilevanza

### 5. bombshell-dev/clack — CLI Prompts
**Cosa è**: Libreria per prompts CLI belli (spinners, select, confirm, multi-select).
**Per nexum**: Pattern UX per `nexum-terminal`. Come rendere l'interazione CLI piacevole.
**Azione**: Ispirazione UX, non dipendenza diretta (è JavaScript).

### 8. mukul975/Anthropic-Cybersecurity-Skills
**Cosa è**: Skill specifiche per security audit/pentest da usare con Claude.
**Per nexum**: Pattern per come strutturare skill domain-specific.
**Azione**: Reference per quando estendiamo il tool system.

### 9. garrytan/gstack — Full-Stack AI Dev
**Cosa è**: Tool per scaffolding AI-powered full-stack apps.
**Per nexum**: Come integra LLM nel workflow di sviluppo — reference architetturale.
**Azione**: Esplorare per idee su integrazione coding.

### 11. shanraisshan/claude-code-best-practice
**Cosa è**: Collection di best practice per usare Claude Code efficacemente.
**Per nexum**: Prompt engineering patterns. Come strutturare system prompts per coding agent.
**Azione**: Leggere e applicare pattern al nostro agent prompt.

### 18. microsoft/presidio — PII Detection
**Cosa è**: Framework Microsoft per detection/anonymization di dati personali (nomi, email, telefoni, SSN, ecc.)
**Per nexum**: Protezione privacy nel memory system:
- Prima di salvare in memoria, detecta e maschera PII
- Compliance GDPR per la persistenza delle conversazioni
- Filtraggio output se contiene PII non richiesto
**Azione**: Valutare integrazione come pre/post-processing nel memory pipeline.

---

## Progetti Non Rilevanti

| Progetto | Motivo |
|----------|--------|
| argyleink/prop-for-that | CSS utility, non correlato |
| temporalio/sdk-java | Java workflow, solo pattern utile |
| baskduf/FableCodex | Game AI, non prioritario |
| Danilaa1/slot-text | Text animation widget |
| ZhuLinsen/daily_stock | Stock trading analysis |
| tashfeenahmed/free-llm-api | API proxy per LLM gratuiti |
| calesthio/OpenMontage | Video editing AI |
| Stirling-Tools/Stirling-PDF | PDF manipulation tool |
| esengine/DeepSeek-Reasonix | Reasoning wrapper (thin) |

---

## Azioni Prioritarie

1. **deer-flow** → Studiare per design `nexum-orchestrator` (multi-agent workflow)
2. **hermes-agent** → Aggiornare vendor submodule, studiare skill system
3. **supermemory** → Pattern per migliorare nexum-memory-retrieval
4. **liteparse** → Integrare per document parsing nel prefill pipeline
5. **BuilderIO/skills** → Pattern per nexum-tools / skill modulari
6. **orca** → Analisi architetturale del loop agent

## Vendor Submodule Status

| Progetto | In vendor/? | Azione |
|----------|-------------|--------|
| hermes-agent | ✅ `nexum_tmp/vendor/hermes-agent` | Aggiornare |
| liteparse | ✅ `nexum_tmp/vendor/liteparse` | Aggiornare |
| supermemory | ✅ `nexum_tmp/vendor/supermemory` | Aggiornare |
| deer-flow | ❌ Non presente | Aggiungere come submodule |
| BuilderIO/skills | ❌ Non presente | Valutare submodule |
| stablyai/orca | ❌ Non presente | Solo analisi, no submodule |
| microsoft/presidio | ❌ Non presente | Valutare per privacy |
