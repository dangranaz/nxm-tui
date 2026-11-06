//! Derived from xai-grok-markdown/src/latex.rs (xai-org/grok-build, Apache-2.0). Modified: stub only (T-latex ticket deferred).
//!
//! LaTeX/math rendering is deferred to a dedicated ticket. These stubs return
//! `None` so that math branches in the parser compile and gracefully degrade
//! (TeX source is shown as-is or highlighted as code).

/// Convert inline TeX math (`$...$`) to Unicode.
/// Stub: returns `None` until the real LaTeX module is implemented.
pub fn latex_to_unicode_inline(_src: &str) -> Option<String> {
    None
}

/// Convert display TeX math (`$$...$$`) to Unicode block lines.
/// Stub: returns `None` until the real LaTeX module is implemented.
pub fn latex_to_unicode_display(_src: &str) -> Option<Vec<String>> {
    None
}

/// Delimitatori LaTeX: stub passthrough (latex è stubbato, normalizzazione
/// differita a un ticket dedicato). Restituisce il chunk così com'è, senza
/// alcuna trasformazione; coerente con "latex off".
#[derive(Clone, Default)]
pub struct LatexDelimiterNormalizer;
impl LatexDelimiterNormalizer {
    pub fn new() -> Self {
        Self
    }
    pub fn reset(&mut self) {}
    /// Passthrough: nessuna normalizzazione LaTeX. Ritorna il chunk così com'è.
    pub fn push(&mut self, chunk: &str) -> String {
        chunk.to_string()
    }
    pub fn finish(&mut self) -> String {
        String::new()
    }
}
