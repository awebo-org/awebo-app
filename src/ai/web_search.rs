//! Lightweight DuckDuckGo HTML search for hint enrichment.
//!
//! Uses the DuckDuckGo HTML endpoint (`html.duckduckgo.com/html/`) which
//! returns plain HTML without JavaScript.  We parse just the result titles
//! and snippets to give the LLM extra context when a command is not found.

use std::time::Duration;

/// A single web search result with title, URL, and snippet.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// Perform a blocking DuckDuckGo search and return up to `max_results`
/// snippets as a single string.  Returns `None` on any error or timeout.
pub fn search(query: &str, max_results: usize) -> Option<String> {
    let results = search_structured(query, max_results)?;
    if results.is_empty() {
        return None;
    }
    Some(
        results
            .iter()
            .map(|r| r.snippet.clone())
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// Perform a blocking DuckDuckGo search and return structured results.
pub fn search_structured(query: &str, max_results: usize) -> Option<Vec<SearchResult>> {
    let body = fetch_search_html(query)?;
    let results = extract_results(&body, max_results);
    if results.is_empty() {
        None
    } else {
        Some(results)
    }
}

/// Fetch the raw HTML from DuckDuckGo. Exposed for reuse.
fn fetch_search_html(query: &str) -> Option<String> {
    let encoded_query = url_encode(query);
    let url = format!("https://html.duckduckgo.com/html/?q={encoded_query}");

    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(8)))
        .build()
        .new_agent();

    let resp = agent
        .get(&url)
        .header("User-Agent", "Mozilla/5.0 (compatible; terminal-ai/1.0)")
        .call()
        .ok()?;

    resp.into_body().read_to_string().ok()
}

/// Minimal URL percent-encoding for the query string.
fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push_str(&format!("{b:02X}"));
            }
        }
    }
    out
}

/// Extract structured results from DuckDuckGo HTML response.
fn extract_results(html: &str, max: usize) -> Vec<SearchResult> {
    let mut results = Vec::new();

    let title_marker = "class=\"result__a\"";
    let snippet_marker = "class=\"result__snippet\"";

    let mut pos = 0;
    while results.len() < max {
        let title_marker_pos = match html[pos..].find(title_marker) {
            Some(p) => pos + p,
            None => break,
        };

        let tag_start = html[..title_marker_pos]
            .rfind('<')
            .unwrap_or(title_marker_pos);
        let href_search_end = (title_marker_pos + title_marker.len() + 200).min(html.len());
        let url = extract_href(&html[tag_start..href_search_end]).unwrap_or_default();

        let title_tag_end = match html[title_marker_pos..].find('>') {
            Some(p) => title_marker_pos + p + 1,
            None => break,
        };
        let title_close = match html[title_tag_end..].find("</") {
            Some(p) => title_tag_end + p,
            None => break,
        };
        let title = strip_html_tags(&html[title_tag_end..title_close])
            .trim()
            .to_string();

        let search_from = title_close;
        let snippet = if let Some(sp) = html[search_from..].find(snippet_marker) {
            let spos = search_from + sp;
            let stag_end = match html[spos..].find('>') {
                Some(p) => spos + p + 1,
                None => {
                    pos = title_close;
                    continue;
                }
            };
            let sclose = match html[stag_end..].find("</") {
                Some(p) => stag_end + p,
                None => {
                    pos = title_close;
                    continue;
                }
            };
            pos = sclose;
            strip_html_tags(&html[stag_end..sclose]).trim().to_string()
        } else {
            pos = title_close;
            String::new()
        };

        if !title.is_empty() {
            results.push(SearchResult {
                title,
                url,
                snippet,
            });
        }
    }

    results
}

/// Extract the href="..." value from a tag fragment.
fn extract_href(tag: &str) -> Option<String> {
    let href_pos = tag.find("href=\"")?;
    let start = href_pos + 6;
    let end = tag[start..].find('"')? + start;
    let raw = &tag[start..end];
    if let Some(ud_pos) = raw.find("uddg=") {
        let val = &raw[ud_pos + 5..];
        Some(url_decode(val.split('&').next().unwrap_or(val)))
    } else {
        Some(raw.to_string())
    }
}

/// Minimal percent-decoding.
fn url_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut bytes = s.bytes();
    while let Some(b) = bytes.next() {
        if b == b'%' {
            let hi = bytes.next().unwrap_or(b'0');
            let lo = bytes.next().unwrap_or(b'0');
            let val = hex_val(hi) * 16 + hex_val(lo);
            out.push(val as char);
        } else if b == b'+' {
            out.push(' ');
        } else {
            out.push(b as char);
        }
    }
    out
}

fn hex_val(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => 0,
    }
}

/// Extract result snippets from DuckDuckGo HTML response (legacy helper).
#[cfg(test)]
fn extract_snippets(html: &str, max: usize) -> Vec<String> {
    extract_results(html, max)
        .into_iter()
        .map(|r| r.snippet)
        .collect()
}

/// Strip HTML tags and decode basic entities from a string.
fn strip_html_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '<' {
            in_tag = true;
            continue;
        }
        if c == '>' {
            in_tag = false;
            continue;
        }
        if in_tag {
            continue;
        }
        if c == '&' {
            let mut entity = String::new();
            for ec in chars.by_ref() {
                if ec == ';' {
                    break;
                }
                entity.push(ec);
                if entity.len() > 8 {
                    break;
                }
            }
            match entity.as_str() {
                "amp" => out.push('&'),
                "lt" => out.push('<'),
                "gt" => out.push('>'),
                "quot" => out.push('"'),
                "apos" => out.push('\''),
                "nbsp" => out.push(' '),
                _ => {
                    out.push('&');
                    out.push_str(&entity);
                    out.push(';');
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_encode_preserves_alphanumeric() {
        assert_eq!(url_encode("hello"), "hello");
        assert_eq!(url_encode("ABC123"), "ABC123");
    }

    #[test]
    fn url_encode_spaces_to_plus() {
        assert_eq!(url_encode("hello world"), "hello+world");
    }

    #[test]
    fn url_encode_special_chars() {
        let encoded = url_encode("a&b=c");
        assert!(encoded.contains("%26"));
        assert!(encoded.contains("%3D"));
    }

    #[test]
    fn url_encode_preserves_unreserved() {
        assert_eq!(url_encode("a-b_c.d~e"), "a-b_c.d~e");
    }

    #[test]
    fn url_encode_empty() {
        assert_eq!(url_encode(""), "");
    }

    #[test]
    fn strip_html_tags_plain_text() {
        assert_eq!(strip_html_tags("hello world"), "hello world");
    }

    #[test]
    fn strip_html_tags_removes_tags() {
        assert_eq!(strip_html_tags("<b>bold</b> text"), "bold text");
    }

    #[test]
    fn strip_html_tags_nested() {
        assert_eq!(strip_html_tags("<div><span>inner</span></div>"), "inner");
    }

    #[test]
    fn strip_html_tags_decodes_entities() {
        assert_eq!(strip_html_tags("a &amp; b"), "a & b");
        assert_eq!(strip_html_tags("&lt;tag&gt;"), "<tag>");
        assert_eq!(strip_html_tags("&quot;quoted&quot;"), "\"quoted\"");
        assert_eq!(strip_html_tags("it&apos;s"), "it's");
        assert_eq!(strip_html_tags("non&nbsp;break"), "non break");
    }

    #[test]
    fn strip_html_tags_unknown_entity() {
        assert_eq!(strip_html_tags("&foo;"), "&foo;");
    }

    #[test]
    fn extract_snippets_empty_html() {
        assert!(extract_snippets("", 5).is_empty());
    }

    #[test]
    fn extract_snippets_no_results() {
        assert!(extract_snippets("<html><body>no results</body></html>", 5).is_empty());
    }

    #[test]
    fn extract_snippets_parses_results() {
        let html = r#"
            <a class="result__a" href="http://example.com/1">First Title</a>
            <div class="result__snippet">First result text</div>
            <a class="result__a" href="http://example.com/2">Second Title</a>
            <div class="result__snippet">Second result text</div>
        "#;
        let snippets = extract_snippets(html, 10);
        assert_eq!(snippets.len(), 2);
        assert_eq!(snippets[0], "First result text");
        assert_eq!(snippets[1], "Second result text");
    }

    #[test]
    fn extract_snippets_respects_max() {
        let html = r#"
            <a class="result__a" href="http://a.com">One Title</a>
            <div class="result__snippet">One</div>
            <a class="result__a" href="http://b.com">Two Title</a>
            <div class="result__snippet">Two</div>
            <a class="result__a" href="http://c.com">Three Title</a>
            <div class="result__snippet">Three</div>
        "#;
        let snippets = extract_snippets(html, 2);
        assert_eq!(snippets.len(), 2);
    }

    #[test]
    fn extract_snippets_skips_empty() {
        let html = r#"
            <a class="result__a" href="http://a.com">  </a>
            <div class="result__snippet">  </div>
            <a class="result__a" href="http://b.com">Real Title</a>
            <div class="result__snippet">Real result</div>
        "#;
        let snippets = extract_snippets(html, 10);
        assert_eq!(snippets.len(), 1);
        assert_eq!(snippets[0], "Real result");
    }

    #[test]
    fn extract_results_returns_structured_data() {
        let html = r#"
            <a class="result__a" href="http://example.com/test">Test Title</a>
            <div class="result__snippet">Test snippet text</div>
        "#;
        let results = extract_results(html, 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Test Title");
        assert_eq!(results[0].snippet, "Test snippet text");
    }

    #[test]
    fn extract_results_decodes_uddg_urls() {
        let html = r#"
            <a class="result__a" href="/l/?uddg=https%3A%2F%2Fen.wikipedia.org%2Fwiki%2FTest&rut=abc">Wiki</a>
            <div class="result__snippet">A test article</div>
        "#;
        let results = extract_results(html, 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://en.wikipedia.org/wiki/Test");
    }

    #[test]
    fn web_search_tool_missing_query() {
        use crate::agent::tools::{Tool, ToolCallArgs, WebSearchTool};
        let tool = WebSearchTool;
        let args = ToolCallArgs::new();
        let result = tool.execute(&args, "/tmp");
        assert!(result.is_error);
        assert!(result.output.contains("missing"));
    }
}
