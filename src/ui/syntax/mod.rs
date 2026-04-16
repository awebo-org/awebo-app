//! Syntax highlighting via tree-sitter grammars.

mod token;

pub use token::TokenKind;

use std::collections::HashMap;
use std::path::Path;

use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

include!(concat!(env!("OUT_DIR"), "/grammars.rs"));

/// Standard highlight capture names used across tree-sitter grammars.
/// Order matters — index into this array is returned in `HighlightEvent::HighlightStart`.
pub const HIGHLIGHT_NAMES: &[&str] = &[
    "attribute",             // 0
    "boolean",               // 1
    "character",             // 2
    "comment",               // 3
    "conditional",           // 4
    "constant",              // 5
    "constant.builtin",      // 6
    "constructor",           // 7
    "escape",                // 8
    "function",              // 9
    "function.builtin",      // 10
    "function.macro",        // 11
    "include",               // 12
    "keyword",               // 13
    "label",                 // 14
    "number",                // 15
    "operator",              // 16
    "property",              // 17
    "punctuation",           // 18
    "punctuation.bracket",   // 19
    "punctuation.delimiter", // 20
    "punctuation.special",   // 21
    "repeat",                // 22
    "string",                // 23
    "string.escape",         // 24
    "string.special",        // 25
    "tag",                   // 26
    "type",                  // 27
    "type.builtin",          // 28
    "variable",              // 29
    "variable.builtin",      // 30
    "variable.parameter",    // 31
    "module",                // 32
    "namespace",             // 33
];

/// Map a highlight name index to our semantic `TokenKind`.
fn index_to_kind(idx: usize) -> TokenKind {
    match idx {
        0 => TokenKind::Attribute,
        1 => TokenKind::Boolean,
        2 => TokenKind::Char,
        3 => TokenKind::Comment,
        4 => TokenKind::ControlFlow,
        5 | 6 => TokenKind::Constant,
        7 => TokenKind::Type,
        8 | 24 => TokenKind::Escape,
        9 | 10 => TokenKind::Function,
        11 => TokenKind::Macro,
        12 => TokenKind::Module,
        13 | 22 => TokenKind::Keyword,
        14 => TokenKind::Label,
        15 => TokenKind::Number,
        16 => TokenKind::Operator,
        17 => TokenKind::Special,
        18..=21 => TokenKind::Punctuation,
        23 | 25 => TokenKind::String,
        26 => TokenKind::Label,
        27 | 28 => TokenKind::Type,
        29 | 31 => TokenKind::Plain,
        30 => TokenKind::Special,
        32 | 33 => TokenKind::Module,
        _ => TokenKind::Plain,
    }
}

/// A highlighted span within source code.
#[derive(Debug, Clone)]
pub struct Token {
    pub start: usize,
    pub end: usize,
    pub kind: TokenKind,
}

struct LanguageEntry {
    config: HighlightConfiguration,
    _name: String,
}

/// A deferred grammar definition — stored at startup, compiled on first use.
struct PendingGrammar {
    name: &'static str,
    extensions: &'static [&'static str],
    language_fn: tree_sitter_language::LanguageFn,
    highlights_query: &'static str,
    injections_query: &'static str,
    locals_query: &'static str,
}

pub struct SyntaxRegistry {
    languages: Vec<LanguageEntry>,
    ext_map: HashMap<String, usize>,
    /// Grammars waiting to be compiled on first access.
    pending: Vec<PendingGrammar>,
    /// Maps extension → pending index (before compilation).
    pending_ext_map: HashMap<String, usize>,
}

impl SyntaxRegistry {
    pub fn new() -> Self {
        Self {
            languages: Vec::new(),
            ext_map: HashMap::new(),
            pending: Vec::new(),
            pending_ext_map: HashMap::new(),
        }
    }

    /// Command: enqueue all vendored grammars for lazy compilation.
    /// No tree-sitter work happens here — grammars are compiled on first use.
    pub fn load_defaults(&mut self) {
        for def in all_grammars() {
            let pidx = self.pending.len();
            for ext in def.extensions {
                self.pending_ext_map.insert(ext.to_lowercase(), pidx);
            }
            self.pending.push(PendingGrammar {
                name: def.name,
                extensions: def.extensions,
                language_fn: def.language_fn,
                highlights_query: def.highlights_query,
                injections_query: def.injections_query,
                locals_query: def.locals_query,
            });
        }
    }

    /// Query: look up config index by file path.
    /// Compiles the grammar lazily if it hasn't been loaded yet.
    pub fn config_for_path(&mut self, path: &Path) -> Option<usize> {
        let ext = path.extension()?.to_str()?.to_lowercase();

        if let Some(&idx) = self.ext_map.get(&ext) {
            return Some(idx);
        }

        let pidx = *self.pending_ext_map.get(&ext)?;
        let pg = &self.pending[pidx];

        let mut config = match HighlightConfiguration::new(
            pg.language_fn.into(),
            pg.name,
            pg.highlights_query,
            pg.injections_query,
            pg.locals_query,
        ) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[syntax] Failed to load {}: {e}", pg.name);
                for ext_key in pg.extensions {
                    self.pending_ext_map.remove(&ext_key.to_lowercase());
                }
                return None;
            }
        };
        config.configure(HIGHLIGHT_NAMES);

        let compiled_idx = self.languages.len();
        for ext_key in pg.extensions {
            let lower = ext_key.to_lowercase();
            self.pending_ext_map.remove(&lower);
            self.ext_map.insert(lower, compiled_idx);
        }
        self.languages.push(LanguageEntry {
            config,
            _name: pg.name.to_string(),
        });

        Some(compiled_idx)
    }

    /// Query: highlight source code, returning token spans.
    /// Uses a thread-local `Highlighter` for zero allocation overhead on repeated calls.
    pub fn highlight(&self, config_idx: usize, source: &[u8]) -> Vec<Token> {
        let entry = match self.languages.get(config_idx) {
            Some(e) => e,
            None => return Vec::new(),
        };

        thread_local! {
            static HL: std::cell::RefCell<Highlighter> =
                std::cell::RefCell::new(Highlighter::new());
        }

        HL.with(|hl| {
            let mut hl = hl.borrow_mut();
            let events = match hl.highlight(&entry.config, source, None, |_| None) {
                Ok(e) => e,
                Err(_) => return Vec::new(),
            };

            let mut tokens = Vec::new();
            let mut kind_stack: Vec<TokenKind> = Vec::new();

            for event in events {
                match event {
                    Ok(HighlightEvent::Source { start, end }) => {
                        let kind = kind_stack.last().copied().unwrap_or(TokenKind::Plain);
                        if start < end {
                            tokens.push(Token { start, end, kind });
                        }
                    }
                    Ok(HighlightEvent::HighlightStart(s)) => {
                        kind_stack.push(index_to_kind(s.0));
                    }
                    Ok(HighlightEvent::HighlightEnd) => {
                        kind_stack.pop();
                    }
                    Err(_) => break,
                }
            }

            tokens
        })
    }
}

#[cfg(test)]
impl SyntaxRegistry {
    /// Query: check if a file extension has a registered or pending grammar.
    pub fn has_language(&self, ext: &str) -> bool {
        let lower = ext.to_lowercase();
        self.ext_map.contains_key(&lower) || self.pending_ext_map.contains_key(&lower)
    }

    /// Query: highlight a single line (convenience wrapper).
    /// `line_start` is the byte offset of this line within the full file.
    pub fn highlight_line(
        &self,
        config_idx: usize,
        full_source: &[u8],
        line_start: usize,
        line_end: usize,
    ) -> Vec<Token> {
        let all = self.highlight(config_idx, full_source);
        all.into_iter()
            .filter(|t| t.end > line_start && t.start < line_end)
            .map(|t| Token {
                start: t.start.saturating_sub(line_start),
                end: t.end.min(line_end).saturating_sub(line_start),
                kind: t.kind,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_registry() -> SyntaxRegistry {
        let mut reg = SyntaxRegistry::new();
        reg.load_defaults();
        reg
    }

    #[test]
    fn loads_rust_grammar() {
        let reg = make_registry();
        assert!(reg.has_language("rs"));
    }

    #[test]
    fn loads_python_grammar() {
        let reg = make_registry();
        assert!(reg.has_language("py"));
    }

    #[test]
    fn unknown_extension_returns_none() {
        let reg = make_registry();
        assert!(!reg.has_language("xyz123"));
    }

    #[test]
    fn highlight_rust_fn() {
        let mut reg = make_registry();
        let idx = reg.config_for_path(Path::new("main.rs")).unwrap();
        let source = b"fn main() {}";
        let tokens = reg.highlight(idx, source);
        assert!(!tokens.is_empty());
        let has_keyword = tokens.iter().any(|t| t.kind == TokenKind::Keyword);
        assert!(has_keyword, "should find 'fn' as keyword, got: {tokens:?}");
    }

    #[test]
    fn highlight_rust_string() {
        let mut reg = make_registry();
        let idx = reg.config_for_path(Path::new("main.rs")).unwrap();
        let source = br#"let x = "hello";"#;
        let tokens = reg.highlight(idx, source);
        let has_string = tokens.iter().any(|t| t.kind == TokenKind::String);
        assert!(has_string, "should find string literal, got: {tokens:?}");
    }

    #[test]
    fn highlight_python() {
        let mut reg = make_registry();
        let idx = reg.config_for_path(Path::new("test.py")).unwrap();
        let source = b"def hello():\n    return 42";
        let tokens = reg.highlight(idx, source);
        let has_keyword = tokens.iter().any(|t| t.kind == TokenKind::Keyword);
        assert!(has_keyword, "should find 'def' as keyword");
    }

    #[test]
    fn no_overlapping_tokens() {
        let mut reg = make_registry();
        let idx = reg.config_for_path(Path::new("main.rs")).unwrap();
        let source = b"fn main() { let x = 42; // comment\n}";
        let tokens = reg.highlight(idx, source);
        for w in tokens.windows(2) {
            assert!(
                w[0].end <= w[1].start,
                "tokens overlap: {:?} and {:?}",
                w[0],
                w[1]
            );
        }
    }

    #[test]
    fn highlight_line_offsets() {
        let mut reg = make_registry();
        let idx = reg.config_for_path(Path::new("main.rs")).unwrap();
        let source = b"fn main() {\n    let x = 42;\n}";
        let line_tokens = reg.highlight_line(idx, source, 12, 27);
        assert!(!line_tokens.is_empty());
        for t in &line_tokens {
            assert!(t.start < 15, "token start {t:?} should be within line");
        }
    }

    #[test]
    fn index_to_kind_covers_all() {
        for i in 0..HIGHLIGHT_NAMES.len() {
            let _ = index_to_kind(i);
        }
    }
}
