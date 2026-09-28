# Vendor TUI Analysis — codice riutilizzabile per nxm-tui

> Ricerca su `<workspace>/nexum/vendors` per migliorare **nxm-tui**
> (TUI Rust pura: `ratatui 0.29` + `crossterm 0.28` + `tokio` + `reqwest`).
> Obiettivo: mappare codice vendor riutilizzabile sui gap noti di nxm-tui, con
> verdetto **Copia** (adattare il sorgente) o **Replica** (reimplementare l'idea).
>
> Analisi read-only. Nessun file di nxm-tui o dei vendor è stato modificato.

## Fatti di base verificati (letti dal sorgente)

- **nxm-tui markdown** (`src/markdown.rs`): parser fatto a mano. Fence ```` ``` ````
  split manuale; inline solo `**bold**`, `*italic*`, `` `code` ``; "syntax
  highlighting" = una singola tinta di foreground per linguaggio (`highlight_style`),
  **nessun tokenizer**. Word-wrap ingenuo che spezza per spazi e **tronca** le linee
  di codice (`&line[..inner_w]`) — non unicode-aware, può panicare su boundary UTF-8.
- **nxm-tui scroll** (`src/history.rs:66`): `Paragraph::new(lines).scroll((scroll_offset, 0))`
  con `scroll_offset: u16` (`src/app.rs:303`). Renderizza **tutte** le linee ogni frame,
  nessuna virtualizzazione, **nessuna ricerca** nello scrollback.
- **nxm-tui `/compact`**: `Command::Compact` esiste (`src/app.rs:130,213`, `handler.rs:230`)
  ma è **placeholder**.
- Nessun diff rendering, nessun hyperlink OSC 8, thinking display limitato.

### Versioni ratatui dei vendor (impatto diretto sulla compatibilità)

| Vendor | ratatui | Conseguenza per nxm-tui (0.29) |
|--------|---------|--------------------------------|
| `codex/codex-rs` | **0.30.2** (`default-features=false`) | API di buffer/cella cambiate (`CellDiffOption`, `CellWidth` in `terminal_hyperlinks.rs`). Copia diretta **non** compatibile → **Replica**. |
| `xai-grok-*` | workspace (`ratatui = { workspace = true }`, feature `unstable-widget-ref`) | Fortemente accoppiati a crate interni (`xai-grok-markdown-core`, `xai-grok-pager-render`, `xai-grok-version`…). Copia dell'intero crate non praticabile → **Replica**, tranne moduli isolati. |
| `llmfit/llmfit-tui` | ratatui standard, usa `List`/`ListState` di libreria | Compatibile; è essenzialmente uso idiomatico di ratatui → **Replica** banale. |

---

## Tabella riassuntiva

| Vendor | Modulo/crate | Feature | Licenza | Copia o Replica | Gap nxm-tui coperto | Effort |
|--------|--------------|---------|---------|-----------------|---------------------|--------|
| grok-build | `xai-grok-markdown` (`syntax.rs`, `open_code_highlighter.rs`, `parse.rs`, `streaming.rs`) | Markdown streaming + syntect (250+ lang via two-face), highlight incrementale O(N) | Apache-2.0 | **Replica** (accoppiato a `xai-grok-markdown-core`) | Markdown basilare / no syntax highlighting | **L** |
| codex | `tui/src/streaming/{render,controller,table_holdback}.rs` | Render markdown incrementale in-flight: prefisso stabile immutabile + tail mutabile, holdback tabelle | Apache-2.0 | **Replica** (ratatui 0.30 + crate interni) | Streaming render incrementale / re-render costoso | **M** |
| grok-build | `xai-grok-markdown/open_code_highlighter.rs` | Cache syntect resumable per fence aperto (evita O(N²) sullo stream) | Apache-2.0 | **Replica** (pattern) | Perf highlight durante streaming | **M** |
| grok-build | `xai-grok-pager/src/search/` (`mod.rs` + `matcher.rs`) | `TextMatcher` smart-case (substring/regex) + `next_/prev_index` con wrap | Apache-2.0 | **COPIA** (self-contained, solo `regex`) | Nessuna ricerca nello scrollback | **S** |
| grok-build | `xai-grok-pager` (list_pane virtualizzata, jump picker) | Scrollback virtualizzato + selezione + jump | Apache-2.0 | **Replica** (crate monolitico, `xai-grok-pager-render`) | Scroll/viewport semplice, no virtualizzazione | **L** |
| grok-build | `xai-ratatui-inline` (`scrollback.rs`, `segment.rs`, `resize.rs`) | Viewport inline + scrollback differenziale + resize | Apache-2.0 | **Replica** (idea del diff dei segmenti) | Viewport `scroll_offset` u16 primitivo | **M** |
| codex | `tui/src/diff_render.rs` | Diff unificato: gutter `+/-/ `, numeri di riga, bg theme-aware, highlight per-hunk | Apache-2.0 | **Replica** (dep `diffy`, `codex-utils-absolute-path`, ratatui 0.30) | Nessun diff rendering | **M** |
| codex | `tui/src/terminal_hyperlinks.rs` | OSC 8 hyperlink separati dalla geometria del testo | Apache-2.0 | **Replica** (usa API buffer ratatui 0.30) | Nessun hyperlink terminale | **S–M** |
| grok-build | `xai-grok-markdown/hyperlinks.rs` + `url_scan.rs` | Rilevamento URL semplici + target hyperlink, emissione OSC 8 | Apache-2.0 | **Replica** | Hyperlink terminale (approccio più portabile di codex) | **S–M** |
| llmfit | `llmfit-tui/src/tui_ui.rs` | Lista modelli virtualizzata via `List`/`ListState`; finestra visibile grapheme-aware per la query | MIT | **Replica** (uso idiomatico ratatui) | Virtualized list / picker | **S** |
| openviking | `ov_cli` / crates | TUI ratatui di riferimento | **Apache-2.0** (vedi §Licenza) | Riferimento | — | — |

Legenda effort: **S** = 1–2 h, **M** = mezza/1 giornata, **L** = 2+ giornate.

---

## Raccomandazioni TOP

### 1. Markdown + syntax highlighting  — *gap #1*

**Fonte:** `vendors/grok-build/crates/codegen/xai-grok-markdown/`
- `src/syntax.rs` — wrapper `Syntect` su `syntect` + `two-face` (250+ linguaggi da `bat`), risoluzione per token o per path/estensione, `syntax_highlight_raw()` che ritorna `Vec<Vec<(SyntectStyle, String)>>` per linea.
- `src/open_code_highlighter.rs` — cache incrementale (vedi §3).
- `src/parse.rs`, `src/streaming.rs`, `src/style.rs`, `src/colors.rs` — parser `pulldown-cmark`, stile, e **adattamento colore** (`adapt_style`, `detect_color_level`) per terminali 256/16 colori.

**Perché non copiare l'intero crate:** dipende da `xai-grok-markdown-core` (crate interno del workspace grok, non presente come pacchetto pubblico) e da `anstyle-syntect`, `linkify`, `html-escape`. È **Replica**.

**Dipendenze da aggiungere a nxm-tui:**
```toml
syntect = "5"          # tokenizer + temi .tmTheme
two-face = "0.4"       # syntax set esteso (250+ lang), opzionale ma consigliato
pulldown-cmark = "0.12" # parser markdown CommonMark (block + inline corretti)
unicode-width = "0.2"  # wrap corretto (rimpiazza il wrap ingenuo attuale)
```
(In alternativa a `two-face`, usare `SyntaxSet::load_defaults_newlines()` di syntect — meno linguaggi ma zero dipendenze extra.)

**Note di adattamento a ratatui 0.29:** `syntect::highlighting::Style` → `ratatui::style::Style` è una conversione RGB manuale (`Color::Rgb(fg.r, fg.g, fg.b)`); il crate grok usa `anstyle-syntect` per farlo — in nxm-tui si scrive una `fn to_ratatui(syntect::Style) -> ratatui::Style` di ~10 righe. Nessuna API ratatui 0.30-only coinvolta: `Line`/`Span`/`Style` sono stabili tra 0.29 e 0.30. Sostituire l'attuale `highlight_style()` (una tinta per lang) e il troncamento `&line[..inner_w]` (bug UTF-8) con wrap `unicode-width`.

**Design da replicare:** parse con `pulldown-cmark` → per ogni fenced code block chiamare `syntax_highlight_raw` → mappare a `Line<'static>`. Portare `colors.rs::adapt_style` per il downgrade su terminali poveri (nxm-tui oggi non lo fa).

**Obblighi di licenza:** Apache-2.0. Copyright `2023-2026 SpaceXAI`. Se si porta codice o si replica testualmente: mantenere l'header di licenza sui file derivati, includere `LICENSE` Apache-2.0 e citare l'attribuzione in un `NOTICE`/`THIRD_PARTY.md`. `syntect`/`two-face` sono MIT.

---

### 2. Incremental streaming render  — *gap #2 (re-render costoso durante SSE)*

**Fonte:** `vendors/codex/codex-rs/tui/src/streaming/`
- `render.rs` — `StreamingRender`: partiziona il sorgente in **prefisso stabile immutabile** (`stable_source_len` / `stable_rendered_len`) e **tail mutabile**; solo l'ultimo blocco top-level viene ri-renderizzato quando arrivano nuovi delta.
- `controller.rs` — modello a due regioni (stable committato allo scrollback, tail transitorio); gestione resize (`set_width`) e invarianti append-only.
- `table_holdback.rs` — "holdback" tabelle: una tabella pipe non è renderizzabile incrementalmente (una nuova riga cambia tutte le larghezze), quindi la si tiene nel tail finché lo stream non finalizza.

**Compatibilità ratatui 0.29:** codex è su **0.30.2** e i file usano crate interni (`history_cell`, `inline_visualization`). Non copiabile → **Replica del pattern**, che è puro algoritmo su `Vec<Line>`/offset e non tocca API instabili.

**Dipendenze da aggiungere:** nessuna (logica pura). Si appoggia al renderer markdown della raccomandazione #1.

**Note di adattamento:** nxm-tui oggi ri-renderizza tutta la history ad ogni delta SSE. Introdurre `stable_len`/`tail` sul buffer del messaggio assistant in streaming: freeze del prefisso ai confini di blocco (riga vuota / fence chiusa), re-render del solo tail. Grande vincita percepita su risposte lunghe.

**Obblighi di licenza:** Apache-2.0 (codex). Copyright OpenAI. Replica dell'idea → citare in `NOTICE`; se si copiano commenti/strutture testuali, mantenere header Apache-2.0.

---

### 3. Highlight incrementale performante durante lo streaming  — *gap #2 (perf)*

**Fonte:** `vendors/grok-build/crates/codegen/xai-grok-markdown/src/open_code_highlighter.rs`

**Cosa fa:** persiste lo stato *resumable* di syntect (`ParseState`/`HighlightState`) attraverso le passate di render del tail, così ogni riga di codice è evidenziata **una sola volta**. Senza questa cache, ri-evidenziare l'intero blocco crescente ad ogni push è **O(N²)** (documentato ~35 ms/push a ~1000 righe, freeze UI ~4.5 s). Include anche un memo per fence chiusi intrappolati in un tail non-freezabile (`CLOSED_MEMO_CAP_BYTES = 256 KiB`).

**Copia o replica:** **Replica** — il file è ottimo come riferimento ma il modulo è `pub(crate)` e legato ai tipi interni del crate. La logica (chiavi `fence_info`/`start_in_tail`/`committed_len`, `committed_prefix_matches` append-only) è portabile in ~150 righe.

**Dipendenze:** `syntect` (già dalla racc. #1). Applicare solo se si adotta sia #1 (syntect) sia #2 (streaming).

**Obblighi di licenza:** Apache-2.0 SpaceXAI (come #1).

---

### 4. Ricerca nello scrollback  — *gap #3*  ✅ COPIA DIRETTA

**Fonte:** `vendors/grok-build/crates/codegen/xai-grok-pager/src/search/`
- `matcher.rs` — `TextMatcher`: query substring o regex, compilate a `regex::Regex`, **smart-case** (case-insensitive salvo maiuscole nella query, alla Vim/ripgrep). `is_error()` per regex malformate, `compiled_regex()` per evidenziare i match.
- `mod.rs` — `next_index_after` / `prev_index_before`: navigazione `n`/`N` con wrap su slice ordinata di posizioni.

**Copia o replica:** **COPIA**. È l'unico pezzo **completamente self-contained** dei vendor grok: dipende solo da `regex`, nessun tipo del workspace, test inclusi. Si può incollare in `src/search.rs`.

**Dipendenze da aggiungere:**
```toml
regex = "1"
```

**Note di adattamento ratatui 0.29:** nessuna — è logica pura, indipendente da ratatui. Serve solo cablare la UI: campo query nella `bottom` bar, evidenziazione dei match nello scrollback (usare `compiled_regex()` sulle `Line`), tasti `n`/`N`.

**Obblighi di licenza:** Apache-2.0 SpaceXAI. Mantenere gli header di licenza sui due file copiati e aggiungere l'attribuzione in `NOTICE`/`THIRD_PARTY.md`.

---

### 5. Diff rendering  — *gap #4*

**Fonte:** `vendors/codex/codex-rs/tui/src/diff_render.rs`

**Cosa fa:** rende diff unificati con numero di riga a destra, gutter `+`/`-`/` `, sfondi **theme-aware** (tinte scure vs pastelli GitHub su chiaro), palette dedicate per truecolor/256/16 colori, e highlight per-hunk (preserva lo stato parser di syntect entro un hunk).

**Compatibilità:** ratatui **0.30** + `diffy` (parsing hunk) + `codex-utils-absolute-path` (crate interno) + `unicode-width`. Non copiabile as-is → **Replica**.

**Dipendenze da aggiungere:**
```toml
diffy = "0.4"          # oppure similar = "2" (usato da grok-pager)
unicode-width = "0.2"
```

**Note di adattamento ratatui 0.29:** il rendering è tutto `Line`/`Span`/`Style` (stabile in 0.29). Rimuovere `codex-utils-absolute-path` (usare `std::path::Path`). Portare le costanti palette (`DARK_TC_ADD_LINE_BG_RGB` ecc.) e la logica `is_light()` per lo sfondo. Non serve la parte di `Buffer` custom.

**Obblighi di licenza:** Apache-2.0 (OpenAI/codex). Attribuzione in `NOTICE`; header Apache-2.0 se si copia il file.

---

### 6. Hyperlink terminale (OSC 8)  — *gap #5*

**Due fonti, entrambe Apache-2.0:**

- **codex** `tui/src/terminal_hyperlinks.rs` — separa i link semantici (`TerminalHyperlink { columns, destination }`) dalla geometria del testo; emette OSC 8 solo quando il testo raggiunge il buffer terminale. **Problema di compat:** usa `ratatui::buffer::{CellDiffOption, CellWidth}` (**API 0.30-only**) → **Replica**, non copia.
- **grok** `xai-grok-markdown/src/hyperlinks.rs` + `url_scan.rs` — `HyperlinkTarget`, rilevamento URL semplici (`detect_plain_urls`) e associazione ai range di colonna. Più portabile perché lavora su `Line`.

**Copia o replica:** **Replica**. L'emissione OSC 8 è banale: `\x1b]8;;{url}\x1b\\{testo}\x1b]8;;\x1b\\`. Il valore aggiunto dei vendor è **tenere i link fuori dalla misura/wrapping** e applicarli solo in fase di scrittura al terminale — pattern da replicare.

**Dipendenze da aggiungere:**
```toml
url = "2"       # validazione destinazioni (opzionale)
linkify = "0.10" # oppure regex per rilevare URL nel testo (grok usa linkify)
```

**Note di adattamento ratatui 0.29:** ratatui 0.29 **non** ha un'API nativa per OSC 8 per-span, quindi l'emissione va fatta scrivendo direttamente le sequenze nella pipeline di output (o via `crossterm`). Non usare l'approccio codex basato su `Buffer` 0.30; usare l'approccio "annota `Line` con range, emetti alla scrittura" del modello grok.

**Obblighi di licenza:** Apache-2.0 (SpaceXAI e/o OpenAI a seconda della fonte replicata). Attribuzione in `NOTICE`.

---

### 7. Lista virtualizzata / picker  — *gap #6*

**Fonte:** `vendors/llmfit/llmfit-tui/src/tui_ui.rs` (MIT)

**Cosa fa:** lista modelli virtualizzata tramite `ratatui::widgets::{List, ListState}` (la virtualizzazione è nativa di ratatui: renderizza solo le righe visibili in base all'`offset` interno di `ListState`). Include `visible_search_query()`: finestra scorrevole **grapheme-aware** (`unicode-segmentation`) per input di ricerca più larghi dell'area.

**Copia o replica:** **Replica** — è uso idiomatico di ratatui, quasi zero codice vendor-specifico da portare. Rilevante come **pattern** per: (a) lista provider/model picker virtualizzata, (b) risultati di ricerca scrollback (racc. #4) in un pannello con `ListState`.

**Dipendenze da aggiungere:**
```toml
unicode-segmentation = "1"   # solo per la finestra query grapheme-aware
```
(`List`/`ListState` sono già in `ratatui 0.29`.)

**Note di adattamento ratatui 0.29:** nessuna — `render_stateful_widget(List, area, &mut ListState)` è identico in 0.29. Anche il viewport dello scrollback di nxm-tui potrebbe migrare da `Paragraph::scroll((u16,0))` a una `List` con `ListState` per avere virtualizzazione "gratis".

**Obblighi di licenza:** MIT (llmfit). Includere la nota di copyright MIT in `THIRD_PARTY.md` se si copia codice; per sola replica dell'idea nessun obbligo stringente (buona pratica citare comunque).

---

## Verifica licenza openviking

- File letto: `vendors/openviking/crates/LICENSE` → **Apache License, Version 2.0**.
- `vendors/openviking/LICENSE` → anch'esso Apache-2.0.
- **Nessun file `NOTICE`** trovato in tutto `vendors/openviking/**` (ricerca `**/NOTICE*` → 0 risultati).
- Sotto `vendors/openviking/third_party/` ci sono licenze di terze parti separate
  (spdlog, leveldb, rapidjson, croaring) — **da non toccare**; non riguardano `ov_cli`.

**Obblighi di attribuzione (Apache-2.0 §4) se si usa/replica codice openviking:**
1. Fornire una copia della licenza Apache-2.0 ai destinatari.
2. Mantenere avvisi di copyright, brevetto, marchio e attribuzione presenti nel sorgente.
3. Segnalare nei file modificati che sono stati modificati.
4. Poiché **non esiste un NOTICE** upstream, non c'è testo NOTICE obbligatorio da propagare; è comunque buona pratica creare in nxm-tui un `THIRD_PARTY.md` che elenchi openviking come fonte Apache-2.0.

> ⚠️ Tutti i crate `xai-grok-*` e `codex` sono **Apache-2.0** (Copyright `2023-2026 SpaceXAI`; OpenAI per codex): stessi obblighi §4. `llmfit-tui` è **MIT** (solo nota di copyright da preservare). `syntect`, `two-face`, `pulldown-cmark`, `regex` sono MIT/Apache-2.0 — nessun problema di compatibilità con la licenza MIT di nxm-tui.

---

## Approfondimento openviking (ratatui 0.29 — stessa versione di nxm-tui)

openviking era il candidato più promettente per **copia diretta di widget** perché
è l'unico vendor su `ratatui 0.29` + `crossterm 0.28` (identico a nxm-tui). Verdetto
dopo lettura del sorgente (`crates/ov_cli/src/`): **valore inferiore all'atteso**,
con **una eccezione preziosa**.

**Perché la maggior parte NON è utile:**
- Dominio = RAG / filesystem (`ragfs`), non chat LLM. `commands/chat.rs` (1596 righe)
  fa streaming SSE ma renderizza in **modalità CLI** (`print!`/`eprint!`,
  `print_stream_event`, `render_chat_banner`) — non un widget di transcript ratatui.
- `tui/ui.rs` usa gli **stessi identici pattern che nxm-tui ha già**:
  `Paragraph::new(..).wrap(Wrap{trim:false}).scroll((offset,0))` e `List`/`ListState`
  con virtualizzazione **manuale** via `.skip(scroll_offset)` (persino meno idiomatica
  dell'offset nativo di `ListState`). Nessun upgrade rispetto a nxm-tui.
- Markdown = **`termimad::MadSkin`** (`chat.rs:1176 fn render_markdown`), che stampa a
  stdout — non produce `Vec<Line>` per ratatui. È una **terza via** (diversa da
  syntect/grok e da pulldown), ma non integrabile nel transcript widget.

**L'eccezione che vale una COPIA — `render_utils.rs::wrap_display_text` (MIT):**
- Word-wrap **unicode-width-aware**: usa `UnicodeWidthChar::width` / `UnicodeWidthStr::width`,
  spezza le parole troppo lunghe **carattere per carattere** rispettando la larghezza di
  cella (CJK/emoji), gestisce `max_lines` con ellipsis ASCII. ~90 righe self-contained
  (`wrap_display_text` + helper `append_word_to_line`), unica dep `unicode-width`.
- **Copre un bug reale di nxm-tui**: l'attuale `markdown.rs` fa `&line[..inner_w]` (slice
  per byte) → tronca male e **può panicare su boundary UTF-8**. `wrap_display_text` è un
  rimpiazzo diretto sicuro.
- **Verdetto: COPIA** (adattare, non è legato a ratatui: ritorna `Vec<String>`). Effort **S**.
  È il **secondo** pezzo copiabile 1:1 individuato, dopo `xai-grok-pager/src/search/`.

**Licenza openviking:** Apache-2.0 (`crates/LICENSE` + root `LICENSE`), nessun `NOTICE`
→ obblighi §4 standard. `wrap_display_text` però è codice applicativo di `ov_cli`; preservare
l'attribuzione Apache-2.0 nel `THIRD-PARTY-NOTICES.md` di nxm-tui.

**Aggiornamento tabella:** openviking passa da _"Riferimento"_ a **_"Copia:
`wrap_display_text`"_** (unicode-aware wrap, S).

---

## Priorità consigliata (impatto / effort)

0. **Wrap unicode-aware — COPIA `openviking wrap_display_text`** *(S, corregge bug UTF-8)*.
   Rimpiazza il wrap per-byte di `markdown.rs` (rischio panic). Indipendente, adottabile subito.
1. **Ricerca scrollback — COPIA `search/`** *(S, alto valore, zero rischio)*.
   Unico altro pezzo copiabile 1:1, solo `regex`, con test. Sblocca subito `n`/`N` e highlight match.
2. **Virtualizzazione viewport con `List`/`ListState`** *(S–M)*.
   Migra `Paragraph::scroll` → `List` (pattern llmfit). Prerequisito naturale per ricerca e scrollback grandi.
3. **Markdown + syntax highlighting (syntect)** *(L, ma è il gap #1 più visibile)*.
   Replica del design `xai-grok-markdown`: `pulldown-cmark` + `syntect` + wrap `unicode-width`. Corregge anche il bug UTF-8 di troncamento attuale.
4. **Streaming render incrementale (prefisso stabile + tail)** *(M)*.
   Replica del pattern codex `streaming/`. Da fare dopo #3 (dipende dal renderer). Elimina il re-render O(N) ad ogni delta SSE.
5. **Hyperlink OSC 8** *(S–M)*.
   Replica approccio grok (annota `Line`, emetti alla scrittura) — evita le API ratatui 0.30 di codex.
6. **Highlight incrementale syntect (cache resumable)** *(M)*.
   Solo se #3+#4 sono attivi e si notano freeze su blocchi di codice lunghi.
7. **Diff rendering** *(M)*.
   Replica codex `diff_render.rs` con `diffy`/`similar`. Priorità legata a quanto i tool agentici produrranno diff.

**Regola trasversale sulla compatibilità:** codex è su **ratatui 0.30** e i crate grok sono monolitici e accoppiati → per nxm-tui (0.29) **tutto è Replica tranne `search/`** (copia diretta). Prima di adottare codice codex che tocca `Buffer`/`Cell`, verificare sempre che non usi API 0.30-only.
