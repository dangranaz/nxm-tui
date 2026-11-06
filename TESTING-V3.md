# Test del renderer V3 (markdown à la grok) — guida rapida

Il nuovo renderer markdown (port fedele di `xai-grok-markdown`) è **cablato ma
spento di default**. Si attiva con una variabile d'ambiente. Il vecchio renderer
(legacy) resta il default, quindi senza il flag non cambia nulla.

## Come scaricare l'ultima versione

```bash
git clone https://github.com/dangranaz/nxm-tui.git   # prima volta
# oppure, se già clonato:
git pull origin main
```

## Come accendere il renderer V3

Il flag è `NXM_USE_NEW_MD` (valori accettati: `1`, `true`, `yes`, `on`;
case-insensitive). Default **OFF** = renderer legacy.

```bash
# Renderer V3 (nuovo, ricco):
NXM_USE_NEW_MD=1 cargo run --release

# Renderer legacy (default, per confronto):
cargo run --release
```

Suggerimento: apri due terminali affiancati, uno con il flag e uno senza, e
confronta la stessa risposta.

## Cosa guardare nel transcript

Chatta e chiedi all'assistente di produrre markdown vario. Osserva:

- **Heading** (`#`, `##`, …) — colori/stile per livello
- **Liste** puntate e numerate
- **Bold** `**x**`, *italic* `*x*`, `inline code`
- **Code fence** ```` ```rust ```` — syntax highlighting per linguaggio
- **Tabelle** GFM — bordi boxati, colonne allineate
- **Link** `[testo](url)` — stilizzati (colore/underline)
- **Prosa lunga** — deve andare a capo dentro il pannello (niente sbordi orizzontali)
- **Blockquote** `> …` — ora non crasha più (era un bug, risolto)

## Note note (comportamenti attesi, non bug — da valutare)

Questi sono punti già identificati, da decidere insieme dopo il test:

1. `**strong**` **non** emette ancora il grassetto (BOLD) nel tema attuale.
2. Le righe di **codice lunghe** vanno a capo anche a metà parola.
3. Le **tabelle** sono riempite con spazi fino alla larghezza piena (padding).
4. I marker ` ``` ` dei **code fence** restano visibili (comportamento upstream
   con pretty-mode).
5. I **link non sono ancora cliccabili** (OSC 8): sono solo stilizzati. Il
   supporto cliccabile è un lavoro successivo.

## Note tecniche

- Lo **streaming incrementale** (render veloce mentre arriva la risposta) è
  copiato e testato, ma **non ancora cablato nel loop live** — arriva in un passo
  successivo (T16), da fare insieme al test runtime.
- Terminali che supportano meglio i colori truecolor: iTerm2, WezTerm, Kitty,
  Ghostty, Alacritty. Su terminali 256/16 colori la resa è più piatta.
- Per disattivare: basta NON passare il flag (o `NXM_USE_NEW_MD=0`).

## Se qualcosa si rompe

Annota: cosa hai chiesto, cosa ti aspettavi, cosa è apparso (screenshot se puoi).
Il renderer V3 è isolato dietro il flag: in caso di problemi, il renderer legacy
(senza flag) resta sempre disponibile e invariato.
