//! Web tools — search the web and fetch page content.

use crate::{Tool, ToolContext, ToolError, ToolResult};
use eagent_protocol::messages::RiskLevel;
use serde_json::{Value, json};
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

const MAX_WEB_RESULTS: usize = 5;
const MAX_WEB_BODY_BYTES: usize = 256 * 1024; // 256 KB

// ---------------------------------------------------------------------------
// HTML helpers
// ---------------------------------------------------------------------------

/// Strip HTML tags from a string, keeping only text content.
pub fn strip_html_tags(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => output.push(ch),
            _ => {}
        }
    }
    output
}

/// Decode common HTML entities.
pub fn html_entity_decode(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
}

/// Check whether a URL points to a public web host (not localhost, private IPs, etc.).
pub fn is_public_web_url(url: &reqwest::Url) -> bool {
    if !matches!(url.scheme(), "http" | "https") {
        return false;
    }

    let Some(host) = url.host_str() else {
        return false;
    };

    if host.eq_ignore_ascii_case("localhost")
        || host.ends_with(".local")
        || host.ends_with(".internal")
    {
        return false;
    }

    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return match ip {
            std::net::IpAddr::V4(addr) => {
                !(addr.is_private()
                    || addr.is_loopback()
                    || addr.is_link_local()
                    || addr.is_broadcast()
                    || addr.is_unspecified()
                    || addr.is_multicast())
            }
            std::net::IpAddr::V6(addr) => {
                !(addr.is_loopback()
                    || addr.is_unspecified()
                    || addr.is_multicast()
                    || addr.is_unique_local()
                    || addr.is_unicast_link_local())
            }
        };
    }

    true
}

/// Extract a plain-text snippet from HTML, collapsing whitespace.
fn extract_text_snippet(html: &str, max_len: usize) -> String {
    let stripped = strip_html_tags(html);
    let normalized = stripped.split_whitespace().collect::<Vec<_>>().join(" ");
    limit_output(html_entity_decode(&normalized), max_len)
}

/// Truncate output to `max_len`, appending a truncation notice.
fn limit_output(mut text: String, max_len: usize) -> String {
    if text.len() > max_len {
        text.truncate(max_len);
        text.push_str("\n...<truncated>");
    }
    text
}

/// Extract a required string field from a JSON value.
fn required_str<'a>(params: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    params
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::InvalidParams(format!("missing required string parameter '{}'", key)))
}

// ---------------------------------------------------------------------------
// DuckDuckGo result parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct SearchResult {
    title: String,
    url: String,
}

fn extract_duckduckgo_results(html: &str, max_results: usize) -> Vec<SearchResult> {
    let mut results = Vec::new();
    let marker = "result__a";
    let mut rest = html;

    while let Some(index) = rest.find(marker) {
        rest = &rest[index..];
        let Some(href_start) = rest.find("href=\"") else {
            break;
        };
        let after_href = &rest[href_start + 6..];
        let Some(href_end) = after_href.find('"') else {
            break;
        };
        let url = html_entity_decode(&after_href[..href_end]);

        let Some(text_start) = after_href[href_end..].find('>') else {
            break;
        };
        let title_region = &after_href[href_end + text_start + 1..];
        let Some(title_end) = title_region.find("</a>") else {
            break;
        };
        let title = html_entity_decode(&strip_html_tags(&title_region[..title_end]));

        if url.starts_with("http") && !title.is_empty() {
            results.push(SearchResult { title, url });
        }
        if results.len() >= max_results {
            break;
        }
        rest = &title_region[title_end..];
    }

    results
}

// ---------------------------------------------------------------------------
// HTTP client helper
// ---------------------------------------------------------------------------

fn build_client() -> Result<reqwest::Client, ToolError> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::limited(3))
        .user_agent("eCode-agent/0.1")
        .build()
        .map_err(|e| ToolError::ExecutionFailed(format!("failed to build HTTP client: {}", e)))
}

async fn fetch_html(client: &reqwest::Client, url: reqwest::Url) -> Result<String, ToolError> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("HTTP request failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(ToolError::ExecutionFailed(format!(
            "request failed with status {}",
            response.status()
        )));
    }

    if let Some(length) = response.content_length() {
        if length > MAX_WEB_BODY_BYTES as u64 {
            return Err(ToolError::ExecutionFailed(
                "response exceeded size limit".into(),
            ));
        }
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    if !content_type.is_empty() && !content_type.starts_with("text/html") {
        return Err(ToolError::ExecutionFailed(
            "response is not HTML".into(),
        ));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("failed to read response body: {}", e)))?;
    if bytes.len() > MAX_WEB_BODY_BYTES {
        return Err(ToolError::ExecutionFailed(
            "response exceeded size limit".into(),
        ));
    }

    Ok(String::from_utf8_lossy(&bytes).to_string())
}

// ---------------------------------------------------------------------------
// WebSearchTool
// ---------------------------------------------------------------------------

pub struct WebSearchTool;

impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web using DuckDuckGo and return up to 5 results with titles, URLs, and text snippets."
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Low
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query."
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of results to return (1-5, default 5)."
                }
            },
            "required": ["query"]
        })
    }

    fn execute(
        &self,
        params: Value,
        _ctx: &ToolContext,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, ToolError>> + Send + '_>> {
        Box::pin(async move {
            let query = required_str(&params, "query")?.trim().to_string();
            if query.is_empty() {
                return Err(ToolError::InvalidParams("query cannot be empty".into()));
            }

            let max_results = params
                .get("max_results")
                .and_then(Value::as_u64)
                .unwrap_or(5)
                .min(MAX_WEB_RESULTS as u64) as usize;

            let client = build_client()?;

            let mut url = reqwest::Url::parse("https://html.duckduckgo.com/html/")
                .map_err(|e| ToolError::ExecutionFailed(format!("URL parse error: {}", e)))?;
            url.query_pairs_mut().append_pair("q", &query);

            let html = fetch_html(&client, url).await?;
            let results = extract_duckduckgo_results(&html, max_results);

            if results.is_empty() {
                return Ok(ToolResult {
                    output: json!("No search results found"),
                    is_error: false,
                });
            }

            let mut output = Vec::new();
            for result in results {
                let snippet = match reqwest::Url::parse(&result.url) {
                    Ok(url) if is_public_web_url(&url) => {
                        match fetch_html(&client, url).await {
                            Ok(body) => extract_text_snippet(&body, 600),
                            Err(_) => String::new(),
                        }
                    }
                    _ => String::new(),
                };
                output.push(format!(
                    "TITLE: {}\nURL: {}\nSNIPPET: {}\n",
                    result.title, result.url, snippet
                ));
            }

            Ok(ToolResult {
                output: json!(output.join("\n")),
                is_error: false,
            })
        })
    }
}

// ---------------------------------------------------------------------------
// WebFetchTool
// ---------------------------------------------------------------------------

pub struct WebFetchTool;

impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Fetch a web page by URL and return its text content. Only public URLs are allowed. Max 256KB response."
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Medium
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch."
                },
                "max_length": {
                    "type": "integer",
                    "description": "Maximum length of the extracted text (default 4000)."
                }
            },
            "required": ["url"]
        })
    }

    fn execute(
        &self,
        params: Value,
        _ctx: &ToolContext,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, ToolError>> + Send + '_>> {
        Box::pin(async move {
            let url_str = required_str(&params, "url")?;
            let max_length = params
                .get("max_length")
                .and_then(Value::as_u64)
                .unwrap_or(4000) as usize;

            let url = reqwest::Url::parse(url_str)
                .map_err(|e| ToolError::InvalidParams(format!("invalid URL: {}", e)))?;

            if !is_public_web_url(&url) {
                return Err(ToolError::PermissionDenied(
                    "only public web URLs are allowed".into(),
                ));
            }

            let client = build_client()?;
            let html = fetch_html(&client, url).await?;
            let text = extract_text_snippet(&html, max_length);

            if text.is_empty() {
                Ok(ToolResult {
                    output: json!("Page returned no extractable text content"),
                    is_error: false,
                })
            } else {
                Ok(ToolResult {
                    output: json!(text),
                    is_error: false,
                })
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- strip_html_tags ------------------------------------------------------

    #[test]
    fn strip_html_tags_removes_tags() {
        assert_eq!(strip_html_tags("<p>Hello</p>"), "Hello");
        assert_eq!(strip_html_tags("<b>bold</b> text"), "bold text");
        assert_eq!(strip_html_tags("no tags here"), "no tags here");
    }

    #[test]
    fn strip_html_tags_handles_nested() {
        assert_eq!(
            strip_html_tags("<div><span>nested</span></div>"),
            "nested"
        );
    }

    #[test]
    fn strip_html_tags_handles_empty() {
        assert_eq!(strip_html_tags(""), "");
        assert_eq!(strip_html_tags("<>"), "");
    }

    #[test]
    fn strip_html_tags_preserves_entities() {
        // strip_html_tags only removes tags, not entities
        assert_eq!(strip_html_tags("a &amp; b"), "a &amp; b");
    }

    // -- html_entity_decode ---------------------------------------------------

    #[test]
    fn html_entity_decode_decodes_entities() {
        assert_eq!(html_entity_decode("&amp;"), "&");
        assert_eq!(html_entity_decode("&lt;script&gt;"), "<script>");
        assert_eq!(html_entity_decode("&quot;hello&quot;"), "\"hello\"");
        assert_eq!(html_entity_decode("it&#x27;s"), "it's");
        assert_eq!(html_entity_decode("it&#39;s"), "it's");
        assert_eq!(html_entity_decode("a&nbsp;b"), "a b");
    }

    #[test]
    fn html_entity_decode_preserves_plain_text() {
        assert_eq!(html_entity_decode("hello world"), "hello world");
    }

    #[test]
    fn html_entity_decode_handles_multiple() {
        assert_eq!(
            html_entity_decode("&lt;a&gt; &amp; &lt;b&gt;"),
            "<a> & <b>"
        );
    }

    // -- is_public_web_url ----------------------------------------------------

    #[test]
    fn is_public_web_url_allows_public() {
        let url = reqwest::Url::parse("https://example.com").unwrap();
        assert!(is_public_web_url(&url));

        let url = reqwest::Url::parse("http://docs.rs/crate").unwrap();
        assert!(is_public_web_url(&url));
    }

    #[test]
    fn is_public_web_url_rejects_localhost() {
        let url = reqwest::Url::parse("http://localhost:8080").unwrap();
        assert!(!is_public_web_url(&url));

        let url = reqwest::Url::parse("http://LOCALHOST/path").unwrap();
        assert!(!is_public_web_url(&url));
    }

    #[test]
    fn is_public_web_url_rejects_private_ips() {
        let url = reqwest::Url::parse("http://192.168.1.1").unwrap();
        assert!(!is_public_web_url(&url));

        let url = reqwest::Url::parse("http://10.0.0.1").unwrap();
        assert!(!is_public_web_url(&url));

        let url = reqwest::Url::parse("http://127.0.0.1").unwrap();
        assert!(!is_public_web_url(&url));

        let url = reqwest::Url::parse("http://172.16.0.1").unwrap();
        assert!(!is_public_web_url(&url));
    }

    #[test]
    fn is_public_web_url_rejects_local_domains() {
        let url = reqwest::Url::parse("http://myhost.local").unwrap();
        assert!(!is_public_web_url(&url));

        let url = reqwest::Url::parse("http://myhost.internal").unwrap();
        assert!(!is_public_web_url(&url));
    }

    #[test]
    fn is_public_web_url_rejects_non_http() {
        let url = reqwest::Url::parse("ftp://example.com").unwrap();
        assert!(!is_public_web_url(&url));

        let url = reqwest::Url::parse("file:///etc/passwd").unwrap();
        assert!(!is_public_web_url(&url));
    }

    // -- extract_duckduckgo_results -------------------------------------------

    #[test]
    fn extract_duckduckgo_results_parses_html() {
        // Mimic real DuckDuckGo HTML structure (title text on same line as tag)
        let html = r#"<a class="result__a" href="https://example.com/page1">Example Page 1</a><a class="result__a" href="https://example.com/page2">Example Page 2</a>"#;
        let results = extract_duckduckgo_results(html, 5);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Example Page 1");
        assert_eq!(results[0].url, "https://example.com/page1");
        assert_eq!(results[1].title, "Example Page 2");
        assert_eq!(results[1].url, "https://example.com/page2");
    }

    #[test]
    fn extract_duckduckgo_results_respects_max() {
        let html = r#"
            <a class="result__a" href="https://a.com">A</a>
            <a class="result__a" href="https://b.com">B</a>
            <a class="result__a" href="https://c.com">C</a>
        "#;
        let results = extract_duckduckgo_results(html, 2);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn extract_duckduckgo_results_empty_html() {
        let results = extract_duckduckgo_results("", 5);
        assert!(results.is_empty());
    }

    // -- extract_text_snippet -------------------------------------------------

    #[test]
    fn extract_text_snippet_strips_and_normalizes() {
        let html = "<p>Hello   <b>world</b></p>  <p>More text</p>";
        let snippet = extract_text_snippet(html, 1000);
        assert_eq!(snippet, "Hello world More text");
    }

    #[test]
    fn extract_text_snippet_truncates() {
        let html = "<p>This is a long text that should be truncated</p>";
        let snippet = extract_text_snippet(html, 10);
        assert!(snippet.len() <= 10 + "\n...<truncated>".len());
        assert!(snippet.contains("...<truncated>"));
    }
}
