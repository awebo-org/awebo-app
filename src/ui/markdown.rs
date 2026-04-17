//! Lightweight Markdown → `Vec<StyledLine>` parser for AI output.
//!
//! Handles the subset of Markdown that LLMs commonly produce:
//!
//! - **bold** / __bold__
//! - *italic* / _italic_
//! - ***bold italic***
//! - `inline code`
//! - ``` fenced code blocks ```
//! - # / ## / ### headers
//! - --- / *** / ___ horizontal rules
//! - - / * / + / 1. list items
//! - > blockquotes
//! - ~~strikethrough~~
//!
//! Designed to be called once on the full AI response after streaming
//! completes — not incremental.

use crate::blocks::{StyledLine, StyledSpan};
use crate::renderer::pixel_buffer::Rgb;

const FG_NORMAL: Rgb = (170, 172, 182);
const FG_BOLD: Rgb = (228, 228, 233);
const FG_ITALIC: Rgb = (190, 190, 200);
const FG_CODE: Rgb = (140, 220, 190);
const FG_CODE_BLOCK: Rgb = (130, 200, 170);
const FG_HEADING: Rgb = (235, 235, 240);
const FG_RULE: Rgb = (70, 72, 80);
const FG_BULLET: Rgb = (200, 200, 210);
const FG_BLOCKQUOTE: Rgb = (140, 140, 160);
const FG_STRIKETHROUGH: Rgb = (120, 120, 135);
const FG_LINK_TEXT: Rgb = (97, 175, 239);
const FG_MATH: Rgb = (180, 200, 140);

/// Parse a raw Markdown string into styled lines.
pub fn parse(text: &str) -> Vec<StyledLine> {
    let lines: Vec<&str> = text.lines().collect();
    let mut result: Vec<StyledLine> = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        if line.trim_start().starts_with("```") {
            i += 1;
            while i < lines.len() {
                if lines[i].trim_start().starts_with("```") {
                    i += 1;
                    break;
                }
                result.push(vec![StyledSpan {
                    text: lines[i].to_string(),
                    fg: FG_CODE_BLOCK,
                    bold: false,
                    italic: false,
                    underline: false,
                    strikethrough: false,
                    code: true,
                    heading_level: 0,
                    horizontal_rule: false,
                }]);
                i += 1;
            }
            continue;
        }

        if line.trim() == "$$" {
            i += 1;
            while i < lines.len() {
                if lines[i].trim() == "$$" {
                    i += 1;
                    break;
                }
                result.push(vec![StyledSpan {
                    text: latex_to_unicode(lines[i]),
                    fg: FG_MATH,
                    bold: false,
                    italic: true,
                    underline: false,
                    strikethrough: false,
                    code: true,
                    heading_level: 0,
                    horizontal_rule: false,
                }]);
                i += 1;
            }
            continue;
        }

        let trimmed = line.trim();
        if is_horizontal_rule(trimmed) {
            result.push(vec![StyledSpan {
                text: String::new(),
                fg: FG_RULE,
                bold: false,
                italic: false,
                underline: false,
                strikethrough: false,
                code: false,
                heading_level: 0,
                horizontal_rule: true,
            }]);
            i += 1;
            continue;
        }

        if let Some((level, content)) = parse_heading(trimmed) {
            let prefix = match level {
                1 => "▎ ",
                2 => "▎ ",
                _ => "  ",
            };
            let hl = level as u8;
            let mut spans = vec![StyledSpan {
                text: prefix.to_string(),
                fg: FG_HEADING,
                bold: true,
                italic: false,
                underline: false,
                strikethrough: false,
                code: false,
                heading_level: hl,
                horizontal_rule: false,
            }];
            for mut s in parse_inline(content, true, false, FG_HEADING) {
                s.heading_level = hl;
                spans.push(s);
            }
            result.push(spans);
            i += 1;
            continue;
        }

        if let Some(rest) = trimmed
            .strip_prefix("> ")
            .or_else(|| if trimmed == ">" { Some("") } else { None })
        {
            let mut spans = vec![StyledSpan {
                text: "┃ ".to_string(),
                fg: FG_BLOCKQUOTE,
                bold: false,
                italic: true,
                underline: false,
                strikethrough: false,
                code: false,
                heading_level: 0,
                horizontal_rule: false,
            }];
            spans.extend(parse_inline(rest, false, true, FG_BLOCKQUOTE));
            result.push(spans);
            i += 1;
            continue;
        }

        if let Some(rest) = strip_unordered_bullet(trimmed) {
            let indent = leading_spaces(line);
            let pad = " ".repeat(indent);
            let mut spans = vec![StyledSpan {
                text: format!("{pad}• "),
                fg: FG_BULLET,
                bold: true,
                italic: false,
                underline: false,
                strikethrough: false,
                code: false,
                heading_level: 0,
                horizontal_rule: false,
            }];
            spans.extend(parse_inline(rest, false, false, FG_NORMAL));
            result.push(spans);
            i += 1;
            continue;
        }

        if let Some((num, rest)) = strip_ordered_bullet(trimmed) {
            let indent = leading_spaces(line);
            let pad = " ".repeat(indent);
            let mut spans = vec![StyledSpan {
                text: format!("{pad}{num}. "),
                fg: FG_BULLET,
                bold: true,
                italic: false,
                underline: false,
                strikethrough: false,
                code: false,
                heading_level: 0,
                horizontal_rule: false,
            }];
            spans.extend(parse_inline(rest, false, false, FG_NORMAL));
            result.push(spans);
            i += 1;
            continue;
        }

        let spans = parse_inline(line, false, false, FG_NORMAL);
        result.push(spans);
        i += 1;
    }

    result
}

/// Parse inline Markdown formatting within a single line.
///
/// Handles: **bold**, *italic*, ***bold italic***, `code`,
///          ~~strikethrough~~, [text](url).
fn parse_inline(
    text: &str,
    parent_bold: bool,
    parent_italic: bool,
    default_fg: Rgb,
) -> Vec<StyledSpan> {
    let mut spans = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut pos = 0;
    let mut buf = String::new();

    macro_rules! flush {
        () => {
            if !buf.is_empty() {
                let flushed = if buf.contains('\\') {
                    latex_to_unicode(&std::mem::take(&mut buf))
                } else {
                    std::mem::take(&mut buf)
                };
                spans.push(StyledSpan {
                    text: flushed,
                    fg: default_fg,
                    bold: parent_bold,
                    italic: parent_italic,
                    underline: false,
                    strikethrough: false,
                    code: false,
                    heading_level: 0,
                    horizontal_rule: false,
                });
            }
        };
    }

    while pos < len {
        if chars[pos] == '`'
            && !matches!(peek(&chars, pos + 1), Some('`'))
            && let Some(end) = find_closing(&chars, pos + 1, '`')
        {
            flush!();
            let code_text: String = chars[pos + 1..end].iter().collect();
            spans.push(StyledSpan {
                text: code_text,
                fg: FG_CODE,
                bold: false,
                italic: false,
                underline: false,
                strikethrough: false,
                code: true,
                heading_level: 0,
                horizontal_rule: false,
            });
            pos = end + 1;
            continue;
        }

        if chars[pos] == '$'
            && peek(&chars, pos + 1) != Some('$')
            && let Some(end) = find_closing(&chars, pos + 1, '$')
            && end > pos + 1
        {
            flush!();
            let raw: String = chars[pos + 1..end].iter().collect();
            let math_text = latex_to_unicode(&raw);
            spans.push(StyledSpan {
                text: math_text,
                fg: FG_MATH,
                bold: false,
                italic: true,
                underline: false,
                strikethrough: false,
                code: true,
                heading_level: 0,
                horizontal_rule: false,
            });
            pos = end + 1;
            continue;
        }

        if chars[pos] == '$'
            && peek(&chars, pos + 1) == Some('$')
            && let Some(end) = find_double_closing(&chars, pos + 2, '$')
        {
            flush!();
            let raw: String = chars[pos + 2..end].iter().collect();
            let math_text = latex_to_unicode(&raw);
            spans.push(StyledSpan {
                text: math_text,
                fg: FG_MATH,
                bold: false,
                italic: true,
                underline: false,
                strikethrough: false,
                code: true,
                heading_level: 0,
                horizontal_rule: false,
            });
            pos = end + 2;
            continue;
        }

        if chars[pos] == '~'
            && peek(&chars, pos + 1) == Some('~')
            && let Some(end) = find_double_closing(&chars, pos + 2, '~')
        {
            flush!();
            let inner: String = chars[pos + 2..end].iter().collect();
            spans.push(StyledSpan {
                text: inner,
                fg: FG_STRIKETHROUGH,
                bold: parent_bold,
                italic: parent_italic,
                underline: false,
                strikethrough: true,
                code: false,
                heading_level: 0,
                horizontal_rule: false,
            });
            pos = end + 2;
            continue;
        }

        if chars[pos] == '*'
            && peek(&chars, pos + 1) == Some('*')
            && peek(&chars, pos + 2) == Some('*')
            && let Some(end) = find_triple_closing(&chars, pos + 3, '*')
        {
            flush!();
            let inner: String = chars[pos + 3..end].iter().collect();
            spans.push(StyledSpan {
                text: inner,
                fg: FG_BOLD,
                bold: true,
                italic: true,
                underline: false,
                strikethrough: false,
                code: false,
                heading_level: 0,
                horizontal_rule: false,
            });
            pos = end + 3;
            continue;
        }

        if (chars[pos] == '*' && peek(&chars, pos + 1) == Some('*'))
            || (chars[pos] == '_' && peek(&chars, pos + 1) == Some('_'))
        {
            let marker = chars[pos];
            if let Some(end) = find_double_closing(&chars, pos + 2, marker) {
                flush!();
                let inner: String = chars[pos + 2..end].iter().collect();
                spans.push(StyledSpan {
                    text: inner,
                    fg: FG_BOLD,
                    bold: true,
                    italic: parent_italic,
                    underline: false,
                    strikethrough: false,
                    code: false,
                    heading_level: 0,
                    horizontal_rule: false,
                });
                pos = end + 2;
                continue;
            }
        }

        if chars[pos] == '*' || chars[pos] == '_' {
            let marker = chars[pos];
            let word_boundary = marker == '*'
                || pos == 0
                || chars[pos - 1].is_whitespace()
                || chars[pos - 1].is_ascii_punctuation();
            if word_boundary && let Some(end) = find_closing(&chars, pos + 1, marker) {
                let end_ok = marker == '*'
                    || end + 1 >= len
                    || chars[end + 1].is_whitespace()
                    || chars[end + 1].is_ascii_punctuation();
                if end_ok && end > pos + 1 {
                    flush!();
                    let inner: String = chars[pos + 1..end].iter().collect();
                    spans.push(StyledSpan {
                        text: inner,
                        fg: FG_ITALIC,
                        bold: parent_bold,
                        italic: true,
                        underline: false,
                        strikethrough: false,
                        code: false,
                        heading_level: 0,
                        horizontal_rule: false,
                    });
                    pos = end + 1;
                    continue;
                }
            }
        }

        if chars[pos] == '['
            && let Some((link_text, _url, end_pos)) = parse_link(&chars, pos)
        {
            flush!();
            spans.push(StyledSpan {
                text: link_text,
                fg: FG_LINK_TEXT,
                bold: parent_bold,
                italic: parent_italic,
                underline: true,
                strikethrough: false,
                code: false,
                heading_level: 0,
                horizontal_rule: false,
            });
            pos = end_pos;
            continue;
        }

        buf.push(chars[pos]);
        pos += 1;
    }

    flush!();
    spans
}

fn peek(chars: &[char], idx: usize) -> Option<char> {
    chars.get(idx).copied()
}

/// Find closing single marker that isn't escaped.
fn find_closing(chars: &[char], start: usize, marker: char) -> Option<usize> {
    let mut i = start;
    while i < chars.len() {
        if chars[i] == '\\' {
            i += 2;
            continue;
        }
        if chars[i] == marker {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Find closing double marker (e.g. `**` or `~~`).
fn find_double_closing(chars: &[char], start: usize, marker: char) -> Option<usize> {
    let mut i = start;
    while i + 1 < chars.len() {
        if chars[i] == '\\' {
            i += 2;
            continue;
        }
        if chars[i] == marker && chars[i + 1] == marker {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Find closing triple marker (e.g. `***`).
fn find_triple_closing(chars: &[char], start: usize, marker: char) -> Option<usize> {
    let mut i = start;
    while i + 2 < chars.len() {
        if chars[i] == '\\' {
            i += 2;
            continue;
        }
        if chars[i] == marker && chars[i + 1] == marker && chars[i + 2] == marker {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Convert common LaTeX math commands to Unicode equivalents.
///
/// Handles `\command` symbols, `\sqrt{…}`, `\frac{…}{…}`, and basic
/// superscript/subscript notation (`^{…}`, `_{…}`).
fn latex_to_unicode(input: &str) -> String {
    let mut result = input.to_string();

    while let Some(pos) = result.find("\\frac{") {
        let after = pos + 6;
        if let Some(mid_brace) = find_brace_close(&result, after) {
            let numer = &result[after..mid_brace];
            let rest = &result[mid_brace + 1..];
            if rest.starts_with('{')
                && let Some(denom_end) = find_brace_close(&result, mid_brace + 2)
            {
                let denom = &result[mid_brace + 2..denom_end];
                let replacement = format!("{numer}/{denom}");
                result = format!(
                    "{}{replacement}{}",
                    &result[..pos],
                    &result[denom_end + 1..]
                );
                continue;
            }
        }
        break;
    }

    while let Some(pos) = result.find("\\sqrt{") {
        let after = pos + 6;
        if let Some(end) = find_brace_close(&result, after) {
            let inner = &result[after..end];
            let replacement = format!("√({inner})");
            result = format!("{}{replacement}{}", &result[..pos], &result[end + 1..]);
        } else {
            break;
        }
    }

    const LATEX_SYMBOLS: &[(&str, &str)] = &[
        ("\\times", "×"),
        ("\\cdot", "·"),
        ("\\div", "÷"),
        ("\\pm", "±"),
        ("\\mp", "∓"),
        ("\\leq", "≤"),
        ("\\geq", "≥"),
        ("\\neq", "≠"),
        ("\\approx", "≈"),
        ("\\equiv", "≡"),
        ("\\sim", "∼"),
        ("\\propto", "∝"),
        ("\\infty", "∞"),
        ("\\sum", "∑"),
        ("\\prod", "∏"),
        ("\\int", "∫"),
        ("\\partial", "∂"),
        ("\\nabla", "∇"),
        ("\\forall", "∀"),
        ("\\exists", "∃"),
        ("\\in", "∈"),
        ("\\notin", "∉"),
        ("\\subset", "⊂"),
        ("\\supset", "⊃"),
        ("\\cup", "∪"),
        ("\\cap", "∩"),
        ("\\emptyset", "∅"),
        ("\\rightarrow", "→"),
        ("\\leftarrow", "←"),
        ("\\Rightarrow", "⇒"),
        ("\\Leftarrow", "⇐"),
        ("\\leftrightarrow", "↔"),
        ("\\to", "→"),
        ("\\alpha", "α"),
        ("\\beta", "β"),
        ("\\gamma", "γ"),
        ("\\delta", "δ"),
        ("\\epsilon", "ε"),
        ("\\zeta", "ζ"),
        ("\\eta", "η"),
        ("\\theta", "θ"),
        ("\\iota", "ι"),
        ("\\kappa", "κ"),
        ("\\lambda", "λ"),
        ("\\mu", "μ"),
        ("\\nu", "ν"),
        ("\\xi", "ξ"),
        ("\\pi", "π"),
        ("\\rho", "ρ"),
        ("\\sigma", "σ"),
        ("\\tau", "τ"),
        ("\\upsilon", "υ"),
        ("\\phi", "φ"),
        ("\\chi", "χ"),
        ("\\psi", "ψ"),
        ("\\omega", "ω"),
        ("\\Gamma", "Γ"),
        ("\\Delta", "Δ"),
        ("\\Theta", "Θ"),
        ("\\Lambda", "Λ"),
        ("\\Pi", "Π"),
        ("\\Sigma", "Σ"),
        ("\\Phi", "Φ"),
        ("\\Psi", "Ψ"),
        ("\\Omega", "Ω"),
        ("\\sqrt", "√"),
        ("\\circ", "∘"),
        ("\\deg", "°"),
        ("\\angle", "∠"),
        ("\\perp", "⊥"),
        ("\\parallel", "∥"),
        ("\\triangle", "△"),
        ("\\star", "★"),
        ("\\ldots", "…"),
        ("\\cdots", "⋯"),
        ("\\dots", "…"),
    ];

    for &(cmd, sym) in LATEX_SYMBOLS {
        result = result.replace(cmd, sym);
    }

    while let Some(pos) = result.find("^{") {
        let after = pos + 2;
        if let Some(end) = find_brace_close(&result, after) {
            let inner = &result[after..end];
            let sup = to_superscript(inner);
            result = format!("{}{sup}{}", &result[..pos], &result[end + 1..]);
        } else {
            break;
        }
    }

    while let Some(pos) = result.find("_{") {
        let after = pos + 2;
        if let Some(end) = find_brace_close(&result, after) {
            let inner = &result[after..end];
            let sub = to_subscript(inner);
            result = format!("{}{sub}{}", &result[..pos], &result[end + 1..]);
        } else {
            break;
        }
    }

    let chars: Vec<char> = result.chars().collect();
    let mut out = String::with_capacity(result.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '^' && i + 1 < chars.len() && chars[i + 1] != '{' {
            let c = chars[i + 1];
            let sup = to_superscript(&c.to_string());
            out.push_str(&sup);
            i += 2;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }

    out
}

/// Find the closing `}` for an opening `{` at `start - 1`, handling nesting.
fn find_brace_close(s: &str, start: usize) -> Option<usize> {
    let mut depth = 1u32;
    for (i, c) in s[start..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(start + i);
                }
            }
            _ => {}
        }
    }
    None
}

fn to_superscript(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '0' => '⁰',
            '1' => '¹',
            '2' => '²',
            '3' => '³',
            '4' => '⁴',
            '5' => '⁵',
            '6' => '⁶',
            '7' => '⁷',
            '8' => '⁸',
            '9' => '⁹',
            '+' => '⁺',
            '-' => '⁻',
            '=' => '⁼',
            '(' => '⁽',
            ')' => '⁾',
            'n' => 'ⁿ',
            'i' => 'ⁱ',
            other => other,
        })
        .collect()
}

fn to_subscript(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '0' => '₀',
            '1' => '₁',
            '2' => '₂',
            '3' => '₃',
            '4' => '₄',
            '5' => '₅',
            '6' => '₆',
            '7' => '₇',
            '8' => '₈',
            '9' => '₉',
            '+' => '₊',
            '-' => '₋',
            '=' => '₌',
            '(' => '₍',
            ')' => '₎',
            'a' => 'ₐ',
            'e' => 'ₑ',
            'o' => 'ₒ',
            'x' => 'ₓ',
            'i' => 'ᵢ',
            'j' => 'ⱼ',
            'n' => 'ₙ',
            'k' => 'ₖ',
            other => other,
        })
        .collect()
}

/// Parse `[text](url)` starting at `[`. Returns (link_text, url, end_pos).
fn parse_link(chars: &[char], start: usize) -> Option<(String, String, usize)> {
    if chars.get(start).copied() != Some('[') {
        return None;
    }
    let text_end = find_closing(chars, start + 1, ']')?;
    let link_text: String = chars[start + 1..text_end].iter().collect();

    if chars.get(text_end + 1).copied() != Some('(') {
        return None;
    }
    let url_end = find_closing(chars, text_end + 2, ')')?;
    let url: String = chars[text_end + 2..url_end].iter().collect();

    Some((link_text, url, url_end + 1))
}

fn is_horizontal_rule(trimmed: &str) -> bool {
    if trimmed.len() < 3 {
        return false;
    }
    let no_spaces: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
    (no_spaces.chars().all(|c| c == '-')
        || no_spaces.chars().all(|c| c == '*')
        || no_spaces.chars().all(|c| c == '_'))
        && no_spaces.len() >= 3
}

fn parse_heading(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();
    let level = trimmed.chars().take_while(|&c| c == '#').count();
    if level == 0 || level > 6 {
        return None;
    }
    let rest = &trimmed[level..];
    if rest.starts_with(' ') || rest.is_empty() {
        Some((level, rest.trim_start()))
    } else {
        None
    }
}

fn strip_unordered_bullet(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if let Some(rest) = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("+ "))
    {
        Some(rest)
    } else {
        None
    }
}

fn strip_ordered_bullet(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim_start();
    let num_end = trimmed.find(|c: char| !c.is_ascii_digit())?;
    if num_end == 0 {
        return None;
    }
    let rest = &trimmed[num_end..];
    if let Some(after_dot) = rest.strip_prefix(". ") {
        Some((&trimmed[..num_end], after_dot))
    } else {
        None
    }
}

fn leading_spaces(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_unchanged() {
        let lines = parse("Hello world");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].len(), 1);
        assert_eq!(lines[0][0].text, "Hello world");
        assert!(!lines[0][0].bold);
    }

    #[test]
    fn bold_text() {
        let lines = parse("This is **bold** text");
        assert_eq!(lines.len(), 1);
        assert!(lines[0].len() >= 3);
        assert_eq!(lines[0][1].text, "bold");
        assert!(lines[0][1].bold);
    }

    #[test]
    fn italic_text() {
        let lines = parse("This is *italic* text");
        assert_eq!(lines.len(), 1);
        assert!(lines[0].len() >= 3);
        assert_eq!(lines[0][1].text, "italic");
        assert!(lines[0][1].italic);
    }

    #[test]
    fn bold_italic() {
        let lines = parse("***both***");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0][0].text, "both");
        assert!(lines[0][0].bold);
        assert!(lines[0][0].italic);
    }

    #[test]
    fn inline_code() {
        let lines = parse("Use `cargo build` here");
        assert_eq!(lines.len(), 1);
        assert!(lines[0].len() >= 3);
        assert_eq!(lines[0][1].text, "cargo build");
        assert!(lines[0][1].code);
        assert_eq!(lines[0][1].fg, FG_CODE);
    }

    #[test]
    fn fenced_code_block() {
        let input = "Before\n```rust\nfn main() {}\n```\nAfter";
        let lines = parse(input);
        assert_eq!(lines.len(), 3);
        assert!(lines[1][0].code);
        assert_eq!(lines[1][0].text, "fn main() {}");
    }

    #[test]
    fn heading_levels() {
        for (level, prefix) in [(1, "# "), (2, "## "), (3, "### ")] {
            let lines = parse(&format!("{prefix}Title"));
            assert_eq!(lines.len(), 1);
            assert!(lines[0][0].bold, "h{level} prefix should be bold");
            assert_eq!(
                lines[0][0].heading_level, level as u8,
                "h{level} heading_level"
            );
        }
    }

    #[test]
    fn horizontal_rule() {
        for hr in ["---", "***", "___", "- - -"] {
            let lines = parse(hr);
            assert_eq!(lines.len(), 1, "HR for '{hr}' should produce 1 line");
            assert!(lines[0][0].horizontal_rule, "HR flag for '{hr}'");
        }
    }

    #[test]
    fn unordered_list() {
        let lines = parse("- first\n- second");
        assert_eq!(lines.len(), 2);
        assert!(lines[0][0].text.contains('•'));
    }

    #[test]
    fn ordered_list() {
        let lines = parse("1. first\n2. second");
        assert_eq!(lines.len(), 2);
        assert!(lines[0][0].text.contains("1."));
    }

    #[test]
    fn blockquote() {
        let lines = parse("> quoted text");
        assert_eq!(lines.len(), 1);
        assert!(lines[0][0].text.contains('┃'));
    }

    #[test]
    fn strikethrough() {
        let lines = parse("~~removed~~");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0][0].text, "removed");
        assert!(lines[0][0].strikethrough);
    }

    #[test]
    fn link() {
        let lines = parse("Click [here](https://example.com) please");
        assert_eq!(lines.len(), 1);
        assert!(lines[0].len() >= 3);
        assert_eq!(lines[0][1].text, "here");
        assert!(lines[0][1].underline);
        assert_eq!(lines[0][1].fg, FG_LINK_TEXT);
    }

    #[test]
    fn mixed_formatting() {
        let lines = parse("**Bold** and *italic* with `code`");
        assert_eq!(lines.len(), 1);
        let bold_span = lines[0].iter().find(|s| s.text == "Bold").unwrap();
        assert!(bold_span.bold);
        let italic_span = lines[0].iter().find(|s| s.text == "italic").unwrap();
        assert!(italic_span.italic);
        let code_span = lines[0].iter().find(|s| s.text == "code").unwrap();
        assert!(code_span.code);
    }

    #[test]
    fn empty_input() {
        let lines = parse("");
        assert_eq!(lines.len(), 0);
    }

    #[test]
    fn multiline_preserves_structure() {
        let input = "Line 1\n\nLine 3";
        let lines = parse(input);
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn inline_math() {
        let lines = parse("The result is $37177328 \\times 3312$.");
        assert_eq!(lines.len(), 1);
        let math = lines[0].iter().find(|s| s.fg == FG_MATH).unwrap();
        assert!(
            math.text.contains("×"),
            "\\times should become ×, got: {}",
            math.text
        );
        assert!(math.code);
        assert!(math.italic);
    }

    #[test]
    fn display_math_inline() {
        let lines = parse("Result: $$E = mc^2$$");
        assert_eq!(lines.len(), 1);
        let math = lines[0]
            .iter()
            .find(|s| s.text.contains("E = mc²"))
            .unwrap();
        assert!(math.code);
        assert!(math.italic);
        assert_eq!(math.fg, FG_MATH);
    }

    #[test]
    fn display_math_block() {
        let input = "Before\n$$\nx^2 + y^2\n$$\nAfter";
        let lines = parse(input);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[1][0].text, "x² + y²");
        assert!(lines[1][0].code);
        assert!(lines[1][0].italic);
        assert_eq!(lines[1][0].fg, FG_MATH);
    }

    #[test]
    fn math_mixed_with_text() {
        let lines = parse("Calculate $a + b$ and then $c \\times d$.");
        assert_eq!(lines.len(), 1);
        let math_spans: Vec<_> = lines[0].iter().filter(|s| s.fg == FG_MATH).collect();
        assert_eq!(math_spans.len(), 2);
        assert_eq!(math_spans[0].text, "a + b");
        assert_eq!(math_spans[1].text, "c × d");
    }

    #[test]
    fn latex_simple_symbols() {
        assert!(latex_to_unicode("a \\times b").contains('×'));
        assert!(latex_to_unicode("a \\div b").contains('÷'));
        assert!(latex_to_unicode("\\pi r^2").contains('π'));
        assert!(latex_to_unicode("\\infty").contains('∞'));
    }

    #[test]
    fn latex_sqrt() {
        let r = latex_to_unicode("\\sqrt{555}");
        assert!(r.contains("√(555)"), "got: {r}");
    }

    #[test]
    fn latex_frac() {
        let r = latex_to_unicode("\\frac{a}{b}");
        assert_eq!(r, "a/b");
    }

    #[test]
    fn latex_superscript() {
        let r = latex_to_unicode("x^{2}");
        assert!(r.contains('²'), "got: {r}");
    }

    #[test]
    fn latex_subscript() {
        let r = latex_to_unicode("x_{n}");
        assert!(r.contains('ₙ'), "got: {r}");
    }

    #[test]
    fn bare_latex_in_text() {
        let lines = parse("Result is 5 \\times 3 = 15");
        assert_eq!(lines.len(), 1);
        let text = &lines[0][0].text;
        assert!(
            text.contains('×'),
            "bare \\times should convert, got: {text}"
        );
    }

    #[test]
    fn bare_sqrt_in_text() {
        let lines = parse("The \\sqrt{64} equals 8");
        assert_eq!(lines.len(), 1);
        let text = &lines[0][0].text;
        assert!(
            text.contains("√(64)"),
            "bare \\sqrt should convert, got: {text}"
        );
    }

    #[test]
    fn latex_greek_letters() {
        let r = latex_to_unicode("\\alpha + \\beta = \\gamma");
        assert!(
            r.contains('α') && r.contains('β') && r.contains('γ'),
            "got: {r}"
        );
    }
}
