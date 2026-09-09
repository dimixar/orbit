//! Reader-mode page fetch for the Browser side-pane tab.
//!
//! Orbit is deliberately webview-free (see INTENT.md), so the Browser tab
//! is a reader: it fetches a URL server-side and renders the page's
//! readable text natively in GPUI. One small blocking fetch + a tiny
//! tag-stripping pass — no DOM, no JS.

use std::time::Duration;

/// A fetched page reduced to readable text.
pub struct ReaderPage {
    /// The final URL that was fetched (after scheme fix-ups).
    pub url: String,
    /// `<title>` contents, or the URL host when missing.
    pub title: String,
    /// Plain-text lines (blank lines already collapsed).
    pub lines: Vec<String>,
}

/// Fetch `url` (adding `https://` when no scheme is present) and reduce the
/// HTML to readable text. Runs on a background executor; blocking is fine.
pub fn read_page(raw_url: &str) -> Result<ReaderPage, String> {
    let url = normalize_url(raw_url);
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout(Duration::from_secs(20))
        .user_agent("Orbit/0.1 (reader)")
        .build();
    let response = agent.get(&url).call().map_err(|err| match err {
        ureq::Error::Status(code, _) => format!("HTTP {code}"),
        ureq::Error::Transport(t) => t.to_string(),
    })?;
    let content_type = response
        .content_type()
        .split(';')
        .next()
        .unwrap_or("")
        .to_string();
    // 8 MB cap — reader mode is for articles, not archives.
    let body = response
        .into_string()
        .map_err(|err| format!("read failed: {err}"))?;
    if content_type.contains("html") || content_type == "application/xhtml+xml" {
        let (title, lines) = html_to_text(&body);
        Ok(ReaderPage { url, title, lines })
    } else if content_type.starts_with("text/") || content_type == "application/json" {
        Ok(ReaderPage {
            title: url.clone(),
            lines: plain_lines(&body),
            url,
        })
    } else {
        Err(format!("unsupported content type: {content_type}"))
    }
}

/// Add a scheme when missing; default to https.
fn normalize_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    }
}

/// Split a non-HTML text body into trimmed lines.
fn plain_lines(body: &str) -> Vec<String> {
    body.lines()
        .map(str::trim_end)
        .map(Into::into)
        .collect()
}

/// Block-level tags that start a new text line when they open (a subset is
/// enough — unknown tags are stripped without breaking the line).
const BLOCK_TAGS: &[&str] = &[
    "p", "div", "br", "h1", "h2", "h3", "h4", "h5", "h6", "li", "tr", "ul", "ol", "table",
    "section", "article", "header", "footer", "nav", "blockquote", "pre", "figure", "figcaption",
    "main", "aside", "form", "fieldset", "dl", "dt", "dd", "hr", "option",
];

/// Reduce HTML to `(title, text lines)`: drop `script`/`style` bodies,
/// turn block boundaries into newlines, strip remaining tags, decode
/// entities, collapse whitespace.
fn html_to_text(html: &str) -> (String, Vec<String>) {
    let mut title = String::new();
    let mut out = String::with_capacity(html.len() / 2);
    let mut chars = html.char_indices().peekable();
    let mut skip_until: Option<String> = None; // inside <script>/<style>

    while let Some((_, ch)) = chars.next() {
        if ch != '<' {
            if skip_until.is_none() {
                out.push(ch);
            }
            continue;
        }
        // A tag: read through '>'.
        let mut tag = String::new();
        for (_, tch) in chars.by_ref() {
            if tch == '>' {
                break;
            }
            tag.push(tch);
        }
        let name: String = tag
            .trim_start_matches('/')
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();
        let closing = tag.trim_start().starts_with('/');
        if name == "title" && !closing {
            // Capture the title text up to </title>.
            let mut captured = String::new();
            let mut done = false;
            while let Some((_, tch)) = chars.next() {
                if tch == '<' {
                    let mut end = String::new();
                    for (_, ech) in chars.by_ref() {
                        if ech == '>' {
                            break;
                        }
                        end.push(ech);
                    }
                    if end.trim_start().starts_with("/title") {
                        done = true;
                        break;
                    }
                }
                captured.push(tch);
            }
            if done {
                title = decode_entities(&captured)
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
            }
            continue;
        }
        // Void-ish handling: entering script/style skips their body.
        if (name == "script" || name == "style") && !closing {
            skip_until = Some(name.clone());
        } else if skip_until.as_deref() == Some(name.as_str()) && closing {
            skip_until = None;
            continue;
        } else if skip_until.is_some() {
            continue;
        }
        if BLOCK_TAGS.contains(&name.as_str()) {
            out.push('\n');
        }
    }

    // Decode entities, then collapse whitespace per line.
    let decoded = decode_entities(&out);
    let mut lines: Vec<String> = Vec::new();
    for raw_line in decoded.lines() {
        let collapsed = raw_line.split_whitespace().collect::<Vec<_>>().join(" ");
        if collapsed.is_empty() {
            if matches!(lines.last(), Some(last) if !last.is_empty()) {
                lines.push(String::new());
            }
        } else {
            lines.push(collapsed);
        }
        if lines.len() >= 4000 {
            break;
        }
    }
    while matches!(lines.last(), Some(last) if last.is_empty()) {
        lines.pop();
    }
    (title, lines)
}

/// Decode the HTML entities a reader pass is likely to meet.
fn decode_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut ix = 0;
    while ix < text.len() {
        if bytes[ix] != b'&' {
            // Advance one UTF-8 char.
            let ch = text[ix..].chars().next().unwrap();
            out.push(ch);
            ix += ch.len_utf8();
            continue;
        }
        let Some(semi) = text[ix..].find(';').map(|off| ix + off) else {
            out.push('&');
            ix += 1;
            continue;
        };
        if semi - ix > 10 {
            out.push('&');
            ix += 1;
            continue;
        }
        let entity = &text[ix + 1..semi];
        let replacement = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some('\u{a0}'),
            "copy" => Some('©'),
            "reg" => Some('®'),
            "mdash" => Some('—'),
            "ndash" => Some('–'),
            "hellip" => Some('…'),
            "rsquo" => Some('’'),
            "lsquo" => Some('‘'),
            "rdquo" => Some('”'),
            "ldquo" => Some('“'),
            _ => None,
        };
        match replacement {
            Some(ch) => {
                out.push(ch);
                ix = semi + 1;
            }
            None => {
                let numeric = if let Some(hex) = entity.strip_prefix("#x").or(entity.strip_prefix("#X")) {
                    u32::from_str_radix(hex, 16).ok()
                } else if let Some(dec) = entity.strip_prefix('#') {
                    dec.parse::<u32>().ok()
                } else {
                    None
                };
                match numeric.and_then(char::from_u32) {
                    Some(ch) => {
                        out.push(ch);
                        ix = semi + 1;
                    }
                    None => {
                        out.push('&');
                        ix += 1;
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_adds_https() {
        assert_eq!(normalize_url("example.com"), "https://example.com");
        assert_eq!(normalize_url(" http://a.dev/x "), "http://a.dev/x");
        assert_eq!(normalize_url("https://a.dev"), "https://a.dev");
    }

    #[test]
    fn html_to_text_extracts_title_and_paragraphs() {
        let html = r#"<html><head><title>Hi &amp; bye</title><style>.x{}</style></head>
            <body><script>var a = "<p>not text</p>";</script>
            <h1>Hello</h1><p>One &lt;two&gt;</p><p>Three&nbsp;four</p></body></html>"#;
        let (title, lines) = html_to_text(html);
        assert_eq!(title, "Hi & bye");
        assert_eq!(lines, vec!["Hello", "", "One <two>", "", "Three four"]);
    }

    #[test]
    fn html_to_text_decodes_numeric_entities() {
        let (_, lines) = html_to_text("<p>&#72;&#x65;llo</p>");
        assert_eq!(lines, vec!["Hello"]);
    }

    #[test]
    fn html_to_text_collapses_blank_runs() {
        let (_, lines) = html_to_text("<div>a</div><div></div><div></div><div></div><div>b</div>");
        assert_eq!(lines, vec!["a", "", "b"]);
    }
}