use anyhow::{Context, Result, anyhow, bail};
use ecode_contracts::orchestration::RuntimeMode;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

const MAX_READ_FILE_BYTES: u64 = 1_024 * 1_024;
const MAX_SEARCH_FILE_BYTES: u64 = 512 * 1_024;
const MAX_COMMAND_TIMEOUT_SECONDS: u64 = 60;
const MAX_COMMAND_LENGTH: usize = 2_000;
const MAX_WEB_RESULTS: usize = 5;
const MAX_WEB_BODY_BYTES: usize = 256 * 1_024;
const SKIPPED_SEARCH_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    ".venv",
    "venv",
    "node_modules",
    "target",
    "dist",
    "build",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LocalAgentDecision {
    Assistant { content: String },
    ToolCall { tool: String, arguments: Value },
}

#[derive(Debug, Clone)]
pub struct LocalAgentExecutor {
    project_root: PathBuf,
    web_search_enabled: bool,
    client: reqwest::Client,
}

impl LocalAgentExecutor {
    pub fn new(project_root: PathBuf, web_search_enabled: bool) -> Self {
        let project_root = project_root.canonicalize().unwrap_or(project_root);
        Self {
            project_root,
            web_search_enabled,
            client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(12))
                .redirect(reqwest::redirect::Policy::limited(3))
                .user_agent("eCode-local-agent/0.1")
                .build()
                .expect("local agent client"),
        }
    }

    pub async fn execute(
        &self,
        tool: &str,
        arguments: &Value,
        runtime_mode: RuntimeMode,
    ) -> Result<String> {
        match tool {
            "list_directory" => {
                let path = arguments.get("path").and_then(Value::as_str).unwrap_or(".");
                self.list_directory(path)
            }
            "read_text_file" => {
                let path = required_str(arguments, "path")?;
                self.read_text_file(path)
            }
            "read_multiple_files" => {
                let paths = arguments
                    .get("paths")
                    .and_then(Value::as_array)
                    .ok_or_else(|| anyhow!("paths must be an array"))?;
                let paths = paths
                    .iter()
                    .map(|value| {
                        value
                            .as_str()
                            .ok_or_else(|| anyhow!("paths must contain strings"))
                    })
                    .collect::<Result<Vec<_>>>()?;
                self.read_multiple_files(&paths)
            }
            "search_files" => {
                let pattern = required_str(arguments, "pattern")?;
                let path = arguments.get("path").and_then(Value::as_str).unwrap_or(".");
                self.search_files(path, pattern)
            }
            "run_command" => {
                if runtime_mode != RuntimeMode::FullAccess {
                    bail!("run_command requires full-access mode for the local provider");
                }
                let command = required_str(arguments, "command")?;
                let timeout_seconds = arguments
                    .get("timeout_seconds")
                    .and_then(Value::as_u64)
                    .unwrap_or(20);
                self.run_command(command, timeout_seconds).await
            }
            "apply_patch" => {
                if runtime_mode != RuntimeMode::FullAccess {
                    bail!("apply_patch requires full-access mode for the local provider");
                }
                self.apply_patch(arguments)
            }
            "web_search" => {
                if !self.web_search_enabled {
                    bail!("web_search is disabled for this thread");
                }
                let query = required_str(arguments, "query")?;
                let max_results = arguments
                    .get("max_results")
                    .and_then(Value::as_u64)
                    .unwrap_or(5)
                    .min(MAX_WEB_RESULTS as u64) as usize;
                self.web_search(query, max_results).await
            }
            "request_user_input" => {
                bail!("request_user_input is not implemented for the local provider yet")
            }
            _ => bail!("unsupported tool: {}", tool),
        }
    }

    fn list_directory(&self, path: &str) -> Result<String> {
        let target = self.resolve_path(path)?;
        let mut entries = fs::read_dir(&target)?
            .map(|entry| {
                let entry = entry?;
                let ty = if entry.file_type()?.is_dir() {
                    "[DIR]"
                } else {
                    "[FILE]"
                };
                Ok(format!("{} {}", ty, entry.file_name().to_string_lossy()))
            })
            .collect::<Result<Vec<_>, std::io::Error>>()?;
        entries.sort();
        Ok(entries.join("\n"))
    }

    fn read_text_file(&self, path: &str) -> Result<String> {
        let target = self.resolve_path(path)?;
        if target.metadata()?.len() > MAX_READ_FILE_BYTES {
            bail!("{} exceeds the read size limit", path);
        }
        let content = fs::read_to_string(&target)
            .with_context(|| format!("Failed to read {}", target.display()))?;
        Ok(limit_output(content, 16_000))
    }

    fn read_multiple_files(&self, paths: &[&str]) -> Result<String> {
        let mut parts = Vec::new();
        for path in paths {
            let content = self.read_text_file(path)?;
            parts.push(format!("FILE: {}\n{}", path, content));
        }
        Ok(parts.join("\n\n"))
    }

    fn search_files(&self, path: &str, pattern: &str) -> Result<String> {
        let target = self.resolve_path(path)?;
        let mut matches = Vec::new();
        self.search_path_recursive(&target, pattern, &mut matches)?;
        if matches.is_empty() {
            Ok("No matches found".to_string())
        } else {
            Ok(matches.into_iter().take(200).collect::<Vec<_>>().join("\n"))
        }
    }

    fn search_path_recursive(
        &self,
        path: &Path,
        pattern: &str,
        matches: &mut Vec<String>,
    ) -> Result<()> {
        if matches.len() >= 200 {
            return Ok(());
        }

        if path.is_dir() {
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                if should_skip_search_entry(&entry.path()) {
                    continue;
                }
                self.search_path_recursive(&entry.path(), pattern, matches)?;
            }
            return Ok(());
        }

        if path
            .metadata()
            .map(|metadata| metadata.len() > MAX_SEARCH_FILE_BYTES)
            .unwrap_or(true)
        {
            return Ok(());
        }

        if let Ok(content) = fs::read_to_string(path) {
            for (index, line) in content.lines().enumerate() {
                if line.contains(pattern) {
                    let rel = path
                        .strip_prefix(&self.project_root)
                        .unwrap_or(path)
                        .display()
                        .to_string();
                    matches.push(format!("{}:{}: {}", rel, index + 1, line.trim()));
                }
            }
        }
        Ok(())
    }

    async fn run_command(&self, command: &str, timeout_seconds: u64) -> Result<String> {
        let trimmed = command.trim();
        if trimmed.is_empty() {
            bail!("command cannot be empty");
        }
        if trimmed.len() > MAX_COMMAND_LENGTH {
            bail!("command exceeds the maximum allowed length");
        }

        let timeout_seconds = timeout_seconds.clamp(1, MAX_COMMAND_TIMEOUT_SECONDS);
        let mut child = Command::new("powershell")
            .arg("-NoProfile")
            .arg("-NonInteractive")
            .arg("-Command")
            .arg(trimmed)
            .current_dir(&self.project_root)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        let mut stdout = child
            .stdout
            .take()
            .context("failed to capture command stdout")?;
        let mut stderr = child
            .stderr
            .take()
            .context("failed to capture command stderr")?;
        let stdout_task = tokio::spawn(async move {
            let mut buffer = Vec::new();
            stdout.read_to_end(&mut buffer).await?;
            Ok::<Vec<u8>, std::io::Error>(buffer)
        });
        let stderr_task = tokio::spawn(async move {
            let mut buffer = Vec::new();
            stderr.read_to_end(&mut buffer).await?;
            Ok::<Vec<u8>, std::io::Error>(buffer)
        });

        let status =
            match tokio::time::timeout(Duration::from_secs(timeout_seconds), child.wait()).await {
                Ok(result) => result?,
                Err(_) => {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    bail!("command timed out after {}s", timeout_seconds);
                }
            };

        let stdout = stdout_task.await.context("stdout task failed")??;
        let stderr = stderr_task.await.context("stderr task failed")??;

        let stdout = String::from_utf8_lossy(&stdout);
        let stderr = String::from_utf8_lossy(&stderr);
        let status = status.code().unwrap_or_default();
        Ok(limit_output(
            format!(
                "exit_code: {}\nstdout:\n{}\nstderr:\n{}",
                status, stdout, stderr
            ),
            16_000,
        ))
    }

    fn apply_patch(&self, arguments: &Value) -> Result<String> {
        let path = required_str(arguments, "path")?;
        let target = self.resolve_path(path)?;
        let mut content = fs::read_to_string(&target)?;
        let edits = arguments
            .get("edits")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("apply_patch requires an edits array"))?;

        for edit in edits {
            let old_text = required_str(edit, "old_text")?;
            let new_text = required_str(edit, "new_text")?;
            if !content.contains(old_text) {
                bail!("old_text not found in {}", path);
            }
            content = content.replacen(old_text, new_text, 1);
        }

        let temp_path = target.with_extension("ecode.tmp");
        fs::write(&temp_path, content)?;
        let _ = fs::remove_file(&target);
        fs::rename(&temp_path, &target)?;
        Ok(format!("Patched {}", path))
    }

    async fn web_search(&self, query: &str, max_results: usize) -> Result<String> {
        let query = query.trim();
        if query.is_empty() {
            bail!("query cannot be empty");
        }

        let mut url = Url::parse("https://html.duckduckgo.com/html/")?;
        url.query_pairs_mut().append_pair("q", query);

        let html = self.fetch_html(url).await?;
        let results = extract_duckduckgo_results(&html, max_results);
        if results.is_empty() {
            return Ok("No search results found".to_string());
        }

        let mut output = Vec::new();
        for result in results {
            let snippet = match Url::parse(&result.url) {
                Ok(url) if is_public_web_url(&url) => match self.fetch_html(url).await {
                    Ok(body) => extract_text_snippet(&body, 600),
                    Err(_) => String::new(),
                },
                _ => String::new(),
            };
            output.push(format!(
                "TITLE: {}\nURL: {}\nSNIPPET: {}\n",
                result.title, result.url, snippet
            ));
        }

        Ok(output.join("\n"))
    }

    async fn fetch_html(&self, url: Url) -> Result<String> {
        let response = self.client.get(url).send().await?;
        if !response.status().is_success() {
            bail!("request failed with status {}", response.status());
        }

        if let Some(length) = response.content_length()
            && length > MAX_WEB_BODY_BYTES as u64
        {
            bail!("response exceeded size limit");
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        if !content_type.is_empty() && !content_type.starts_with("text/html") {
            bail!("response is not html");
        }

        let bytes = response.bytes().await?;
        if bytes.len() > MAX_WEB_BODY_BYTES {
            bail!("response exceeded size limit");
        }

        Ok(String::from_utf8_lossy(&bytes).to_string())
    }

    fn resolve_path(&self, path: &str) -> Result<PathBuf> {
        let joined = if Path::new(path).is_absolute() {
            PathBuf::from(path)
        } else {
            self.project_root.join(path)
        };
        let normalized = joined
            .canonicalize()
            .or_else(|_| {
                joined
                    .parent()
                    .map(Path::to_path_buf)
                    .ok_or_else(|| std::io::Error::other("invalid path"))
                    .and_then(|parent| {
                        parent
                            .canonicalize()
                            .map(|resolved| resolved.join(joined.file_name().unwrap_or_default()))
                    })
            })
            .with_context(|| format!("Failed to resolve path {}", joined.display()))?;

        if !normalized.starts_with(&self.project_root) {
            bail!("path escapes project root: {}", path);
        }

        Ok(normalized)
    }
}

pub fn local_agent_system_prompt(web_search_enabled: bool) -> String {
    let web_search_line = if web_search_enabled {
        "- `web_search`: search the web and fetch short text snippets.\n"
    } else {
        ""
    };

    format!(
        "You are a local coding agent. Reply with strict JSON only.\n\
If you need a tool, reply with {{\"type\":\"tool_call\",\"tool\":\"name\",\"arguments\":{{...}}}}.\n\
If you are ready to answer the user, reply with {{\"type\":\"assistant\",\"content\":\"...\"}}.\n\
Available tools:\n\
- `list_directory`: arguments {{\"path\":\"relative/path\"}}\n\
- `read_text_file`: arguments {{\"path\":\"relative/path\"}}\n\
- `read_multiple_files`: arguments {{\"paths\":[\"a\",\"b\"]}}\n\
- `search_files`: arguments {{\"path\":\"relative/path\",\"pattern\":\"text\"}}\n\
- `run_command`: arguments {{\"command\":\"...\",\"timeout_seconds\":20}}\n\
- `apply_patch`: arguments {{\"path\":\"relative/path\",\"edits\":[{{\"old_text\":\"...\",\"new_text\":\"...\"}}]}}\n\
{}\
- `request_user_input`: not currently available, avoid unless absolutely necessary.\n\
Use tools only when needed, keep answers concise, and never invent tool results.",
        web_search_line
    )
}

pub fn parse_local_agent_decision(raw: &str) -> Result<LocalAgentDecision> {
    let trimmed = raw.trim();
    if let Ok(parsed) = serde_json::from_str(trimmed) {
        return Ok(parsed);
    }

    let stripped = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .and_then(|body| body.strip_suffix("```"))
        .map(str::trim)
        .unwrap_or(trimmed);

    serde_json::from_str(stripped)
        .map_err(|e| anyhow!("failed to parse local agent decision: {}", e))
}

fn required_str<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing string argument `{}`", key))
}

fn should_skip_search_entry(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| SKIPPED_SEARCH_DIRS.contains(&name))
}

fn is_public_web_url(url: &Url) -> bool {
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

fn limit_output(mut value: String, max_len: usize) -> String {
    if value.len() > max_len {
        value.truncate(max_len);
        value.push_str("\n...<truncated>");
    }
    value
}

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

fn extract_text_snippet(html: &str, max_len: usize) -> String {
    let stripped = strip_html_tags(html);
    let normalized = stripped.split_whitespace().collect::<Vec<_>>().join(" ");
    limit_output(html_entity_decode(&normalized), max_len)
}

fn strip_html_tags(input: &str) -> String {
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

fn html_entity_decode(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tool_call_json() {
        let raw =
            r#"{"type":"tool_call","tool":"read_text_file","arguments":{"path":"src/main.rs"}}"#;
        let decision = parse_local_agent_decision(raw).unwrap();
        assert!(matches!(decision, LocalAgentDecision::ToolCall { .. }));
    }

    #[test]
    fn apply_patch_replaces_text() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let file = root.join("test.txt");
        fs::write(&file, "hello world").unwrap();
        let executor = LocalAgentExecutor::new(root, true);
        let args = serde_json::json!({
            "path": "test.txt",
            "edits": [{"old_text":"world","new_text":"rust"}]
        });
        executor.apply_patch(&args).unwrap();
        assert_eq!(fs::read_to_string(file).unwrap(), "hello rust");
    }
}
