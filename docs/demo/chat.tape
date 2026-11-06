# VHS tape — nxm-tui live against Ollama. Produces real PNG screenshots.
# Run from the repo root (warm Ollama first):  vhs docs/demo/chat.tape

Output docs/demo/out/chat.gif

Set Shell "zsh"
Set FontSize 18
Set Width 1400
Set Height 860
Set Padding 24
Set Theme "TokyoNight"

Type "NEXUM_LOG=off ./target/release/nxm-tui"
Enter
Sleep 9s

# Warm-up turn: the first send during UI setup can be dropped, so we spend a
# throwaway round-trip here, then screenshot a clean connected state afterwards.
Type@90ms "hi"
Sleep 800ms
Enter
Sleep 12s
Screenshot docs/demo/out/01-connected.png

# Real turn 1 — one-sentence description
Type@90ms "Explain what nxm-tui does in one sentence."
Sleep 1500ms
Enter
Sleep 22s
Screenshot docs/demo/out/02-answer.png

# Real turn 2 — code generation
Type@90ms "Write a Rust function that reverses a string."
Sleep 1500ms
Enter
Sleep 28s
Screenshot docs/demo/out/03-code.png

Ctrl+C
Sleep 1s
