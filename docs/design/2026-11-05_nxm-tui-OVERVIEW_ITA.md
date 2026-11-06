# nxm-tui — Overview (work-in-progress)

> Client chat da terminale per qualsiasi server LLM compatibile OpenAI. Mirror pubblico WIP di `dangranaz/nxm-tui`.

## In una riga

Un client TUI da terminale, guidato da tastiera, che si connette a qualsiasi server LLM compatibile OpenAI (Nexum Inferentia, Ollama, LM Studio, NVIDIA NIM) e streama risposte chat con tool call agentiche, gestione chiavi per provider e un loop agentico bounded — niente gestione server, niente segreti su disco.

## Cosa fa

- **Si connette a qualsiasi provider** — preset built-in per Nexum Inferentia, Ollama, LM Studio, NVIDIA NIM; aggiungi provider custom con `/provider add`.
- **Streaming chat** — token SSE streamati con rendering consapevole della larghezza cella (CJK/emoji safe), Markdown V3 (port da grok-build) ed evidenziazione sintassi.
- **Tool agentici** — `read_file`, `list_resources`, `web_search`, `grep`, `search_replace` tramite `nexum-tools`, con overlay abilita/disabilita per tool e gating di approvazione (Sì/No).
- **Loop bounded** — cap rigido sulle iterazioni (`DEFAULT_MAX_STEPS = 15`) più breaker di stagnazione (`DEFAULT_STALL_THRESHOLD = 3`) fermano modelli che chiamano tool in loop; `/continue` riprende con un Agent nuovo e lo stesso transcript.
- **Segreti al sicuro** — seam `KeyStore` iniettabile (`EnvStore`, `OsKeychainStore`, `EncryptedFileStore` AES-256-GCM + Argon2id); chiavi avvolte in `Zeroizing`, mai loggate, mai scritte su disco se non memorizzate esplicitamente.
- **Host MCP** — genera server MCP stdio, serve `sampling/createMessage` collegandolo al backend LLM; clean no-op quando assente.
- **Overlay** — ragionamento (`Ctrl+R`), dettaglio tool (`Ctrl+O`), metriche (`Ctrl+M`), strumenti (`Ctrl+E`), todo (`Ctrl+D`).

## Stato

Work-in-progress. Su `main` c'è l'esecuzione bounded (loopeng-01-01). Circuit breaker, continue-after-halt, selezione tool per-task e tool todo sono su branch feature in attesa di review esterna (assi Standards + Spec) prima del merge `--ff-only`.

## Provalo

```bash
curl -fsSL https://raw.githubusercontent.com/dangranaz/nxm-tui/main/install.sh | sh
nxm-tui
```

O build da sorgente:

```bash
git clone https://github.com/dangranaz/nxm-tui.git
cd nxm-tui
cargo build --release
./target/release/nxm-tui
```

## Riferimento design

Documento completo: `docs/design/2026-11-05_nxm-tui-DESIGN_ENG.md` (ENG) / `..._DESIGN_ITA.md` (ITA).
