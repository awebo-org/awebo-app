//! Token kinds for syntax highlighting.
//!
//! These are semantic categories — color mapping happens via `to_color()`,
//! keeping the tokenizer independent of any theme system.

use crate::renderer::theme;

type Rgb = (u8, u8, u8);

/// Semantic token category — maps to a color in the editor renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    /// Default text — no special highlighting.
    Plain,
    /// Language keyword (`fn`, `let`, `if`, `for`, …).
    Keyword,
    /// Control flow keyword (`return`, `break`, `continue`, …).
    ControlFlow,
    /// Built-in type (`i32`, `String`, `bool`, …).
    Type,
    /// Numeric literal (`42`, `3.14`, `0xff`).
    Number,
    /// String literal (including delimiters).
    String,
    /// Character literal.
    Char,
    /// Comment (line or block).
    Comment,
    /// Operator (`+`, `-`, `=>`, `::`, …).
    Operator,
    /// Punctuation (`,`, `;`, `{`, `}`, …).
    Punctuation,
    /// Function / method name.
    Function,
    /// Macro invocation (Rust `println!`, C `#define`).
    Macro,
    /// Attribute / annotation / decorator (`#[derive]`, `@Override`).
    Attribute,
    /// Constant / enum variant (UPPER_SNAKE or PascalCase by convention).
    Constant,
    /// Module / namespace / package.
    Module,
    /// Boolean literal (`true`, `false`).
    Boolean,
    /// Special value (`nil`, `null`, `None`, `self`).
    Special,
    /// Label / tag.
    Label,
    /// Escape sequence inside a string.
    Escape,
}

impl TokenKind {
    /// Query: map this token kind to an editor theme color.
    pub fn to_color(self) -> Rgb {
        match self {
            Self::Plain       => theme::FG_PRIMARY,
            Self::Keyword
            | Self::ControlFlow => (198, 120, 221),
            Self::Type          => (229, 192, 123),
            Self::Number        => (209, 154, 102),
            Self::String
            | Self::Char        => (152, 195, 121),
            Self::Comment       => (92, 99, 112),
            Self::Operator      => (86, 182, 194),
            Self::Punctuation   => (171, 178, 191),
            Self::Function      => (97, 175, 239),
            Self::Macro         => (224, 108, 117),
            Self::Attribute     => (229, 192, 123),
            Self::Constant      => (209, 154, 102),
            Self::Module        => (97, 175, 239),
            Self::Boolean       => (209, 154, 102),
            Self::Special       => (224, 108, 117),
            Self::Label         => (86, 182, 194),
            Self::Escape        => (86, 182, 194),
        }
    }
}

#[cfg(test)]
impl TokenKind {
    /// Parse from highlight capture name strings.
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "keyword" => Self::Keyword,
            "control_flow" | "controlflow" => Self::ControlFlow,
            "type" => Self::Type,
            "number" => Self::Number,
            "string" => Self::String,
            "char" => Self::Char,
            "comment" => Self::Comment,
            "operator" => Self::Operator,
            "punctuation" => Self::Punctuation,
            "function" => Self::Function,
            "macro" => Self::Macro,
            "attribute" => Self::Attribute,
            "constant" => Self::Constant,
            "module" => Self::Module,
            "boolean" => Self::Boolean,
            "special" => Self::Special,
            "label" => Self::Label,
            "escape" => Self::Escape,
            _ => Self::Plain,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_str_keyword() {
        assert_eq!(TokenKind::from_str("keyword"), TokenKind::Keyword);
        assert_eq!(TokenKind::from_str("Keyword"), TokenKind::Keyword);
        assert_eq!(TokenKind::from_str("KEYWORD"), TokenKind::Keyword);
    }

    #[test]
    fn from_str_control_flow() {
        assert_eq!(TokenKind::from_str("control_flow"), TokenKind::ControlFlow);
        assert_eq!(TokenKind::from_str("controlflow"), TokenKind::ControlFlow);
    }

    #[test]
    fn from_str_unknown_returns_plain() {
        assert_eq!(TokenKind::from_str("nonexistent"), TokenKind::Plain);
        assert_eq!(TokenKind::from_str(""), TokenKind::Plain);
    }

    #[test]
    fn all_kinds_roundtrip() {
        let cases = [
            ("keyword", TokenKind::Keyword),
            ("type", TokenKind::Type),
            ("number", TokenKind::Number),
            ("string", TokenKind::String),
            ("comment", TokenKind::Comment),
            ("operator", TokenKind::Operator),
            ("function", TokenKind::Function),
            ("macro", TokenKind::Macro),
            ("attribute", TokenKind::Attribute),
            ("boolean", TokenKind::Boolean),
            ("module", TokenKind::Module),
        ];
        for (s, expected) in cases {
            assert_eq!(TokenKind::from_str(s), expected);
        }
    }
}
