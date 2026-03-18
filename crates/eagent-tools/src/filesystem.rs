//! Filesystem tools — list, read, write, edit, search, and patch files.

use crate::{Tool, ToolContext, ToolError, ToolResult};
use eagent_protocol::messages::RiskLevel;
use serde_json::{Value, json};
use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;

const MAX_READ_FILE_BYTES: u64 = 1_024 * 1_024; // 1 MB
const MAX_SEARCH_FILE_BYTES: u64 = 512 * 1_024; // 512 KB
const MAX_OUTPUT_BYTES: usize = 64_000;
const MAX_SEARCH_MATCHES: usize = 200;

const SKIPPED_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    ".venv",
    "venv",
    "node_modules",
    "target",
    "dist",
    "build",
    "__pycache__",
    ".next",
    ".nuxt",
];

// ---------------------------------------------------------------------------
// Security helpers
// ---------------------------------------------------------------------------

/// Resolve a user-supplied path against the workspace root, ensuring it does
/// not escape the workspace boundary.
///
/// Handles three cases:
/// 1. Path exists on disk — canonicalize directly.
/// 2. Path does not exist but parent does — canonicalize parent + file name.
/// 3. Deeply non-existent path (e.g. `sub/deep/file.txt` where `sub` doesn't
///    exist) — normalize logically and verify containment.
pub(crate) fn resolve_path(workspace_root: &str, path: &str) -> Result<PathBuf, ToolError> {
    let root = PathBuf::from(workspace_root);
    let canon_root = root
        .canonicalize()
        .map_err(|e| ToolError::ExecutionFailed(format!("workspace root is invalid: {}", e)))?;

    let joined = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        root.join(path)
    };

    // Fast path: file or directory exists.
    if let Ok(canonical) = joined.canonicalize() {
        if !canonical.starts_with(&canon_root) {
            return Err(ToolError::PermissionDenied(format!(
                "path escapes workspace root: {}",
                path
            )));
        }
        return Ok(canonical);
    }

    // File doesn't exist — try canonicalizing the parent.
    if let Some(parent) = joined.parent() {
        if let Ok(canon_parent) = parent.canonicalize() {
            let normalized = canon_parent.join(joined.file_name().unwrap_or_default());
            if !normalized.starts_with(&canon_root) {
                return Err(ToolError::PermissionDenied(format!(
                    "path escapes workspace root: {}",
                    path
                )));
            }
            return Ok(normalized);
        }
    }

    // Parent also doesn't exist (deeply nested new path). Walk the components
    // logically: resolve from the canon_root, rejecting ".." that would escape.
    let relative = if Path::new(path).is_absolute() {
        // For absolute paths that don't exist and whose parent doesn't exist,
        // just reject — we can't safely resolve them.
        return Err(ToolError::PermissionDenied(format!(
            "path escapes workspace root: {}",
            path
        )));
    } else {
        Path::new(path)
    };

    let mut normalized = canon_root.clone();
    for component in relative.components() {
        match component {
            std::path::Component::ParentDir => {
                // Going up — check we stay within root.
                if !normalized.pop() || !normalized.starts_with(&canon_root) {
                    return Err(ToolError::PermissionDenied(format!(
                        "path escapes workspace root: {}",
                        path
                    )));
                }
            }
            std::path::Component::Normal(c) => {
                normalized.push(c);
            }
            std::path::Component::CurDir => { /* skip */ }
            _ => {
                return Err(ToolError::InvalidParams(format!(
                    "unexpected path component in '{}'",
                    path
                )));
            }
        }
    }

    if !normalized.starts_with(&canon_root) {
        return Err(ToolError::PermissionDenied(format!(
            "path escapes workspace root: {}",
            path
        )));
    }

    Ok(normalized)
}

/// Returns `true` if the directory entry should be skipped during traversal.
fn should_skip_entry(name: &str) -> bool {
    SKIPPED_DIRS.contains(&name)
}

/// Truncate output to `max_bytes`, appending a truncation notice.
/// Uses `is_char_boundary` to avoid panicking on multi-byte UTF-8 characters.
fn limit_output(text: String, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text;
    }
    // Find the nearest char boundary at or before max_bytes
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...\n[output truncated]", &text[..end])
}

/// Extract a required string field from a JSON value.
pub(crate) fn required_str<'a>(params: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    params
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::InvalidParams(format!("missing required string parameter '{}'", key)))
}

// ---------------------------------------------------------------------------
// ListDirectoryTool
// ---------------------------------------------------------------------------

pub struct ListDirectoryTool;

impl Tool for ListDirectoryTool {
    fn name(&self) -> &str {
        "list_directory"
    }

    fn description(&self) -> &str {
        "List files and directories recursively within the workspace."
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Low
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Relative path within the workspace to list. Defaults to workspace root."
                }
            }
        })
    }

    fn execute(
        &self,
        params: Value,
        ctx: &ToolContext,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, ToolError>> + Send + '_>> {
        let workspace_root = ctx.workspace_root.clone();
        Box::pin(async move {
            let path = params
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or(".");
            let target = resolve_path(&workspace_root, path)?;

            let mut entries = Vec::new();
            list_directory_recursive(&target, &target, &mut entries, 0)
                .map_err(|e| ToolError::ExecutionFailed(format!("failed to list directory: {}", e)))?;

            entries.sort();
            let output = limit_output(entries.join("\n"), MAX_OUTPUT_BYTES);
            Ok(ToolResult {
                output: json!(output),
                is_error: false,
            })
        })
    }
}

fn list_directory_recursive(
    base: &Path,
    dir: &Path,
    entries: &mut Vec<String>,
    depth: usize,
) -> std::io::Result<()> {
    if depth > 10 || entries.len() > 2000 {
        return Ok(());
    }

    let mut dir_entries: Vec<_> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .collect();
    dir_entries.sort_by_key(|e| e.file_name());

    for entry in dir_entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if should_skip_entry(&name) {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(base)
            .unwrap_or(&entry.path())
            .to_string_lossy()
            .to_string();

        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        if is_dir {
            entries.push(format!("[DIR]  {}", rel));
            list_directory_recursive(base, &entry.path(), entries, depth + 1)?;
        } else {
            entries.push(format!("[FILE] {}", rel));
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// ReadFileTool
// ---------------------------------------------------------------------------

pub struct ReadFileTool;

impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a text file within the workspace."
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Low
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file (relative to workspace root)."
                }
            },
            "required": ["path"]
        })
    }

    fn execute(
        &self,
        params: Value,
        ctx: &ToolContext,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, ToolError>> + Send + '_>> {
        let workspace_root = ctx.workspace_root.clone();
        Box::pin(async move {
            let path = required_str(&params, "path")?;
            let target = resolve_path(&workspace_root, path)?;

            let metadata = target
                .metadata()
                .map_err(|e| ToolError::ExecutionFailed(format!("cannot stat '{}': {}", path, e)))?;

            if metadata.len() > MAX_READ_FILE_BYTES {
                return Err(ToolError::ExecutionFailed(format!(
                    "'{}' exceeds the 1 MB read limit ({} bytes)",
                    path,
                    metadata.len()
                )));
            }

            let content = fs::read_to_string(&target).map_err(|e| {
                ToolError::ExecutionFailed(format!("failed to read '{}': {}", path, e))
            })?;
            let content = limit_output(content, MAX_OUTPUT_BYTES);
            Ok(ToolResult {
                output: json!(content),
                is_error: false,
            })
        })
    }
}

// ---------------------------------------------------------------------------
// ReadMultipleFilesTool
// ---------------------------------------------------------------------------

pub struct ReadMultipleFilesTool;

impl Tool for ReadMultipleFilesTool {
    fn name(&self) -> &str {
        "read_multiple_files"
    }

    fn description(&self) -> &str {
        "Read the contents of multiple text files within the workspace."
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Low
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "paths": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Array of file paths (relative to workspace root)."
                }
            },
            "required": ["paths"]
        })
    }

    fn execute(
        &self,
        params: Value,
        ctx: &ToolContext,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, ToolError>> + Send + '_>> {
        let workspace_root = ctx.workspace_root.clone();
        Box::pin(async move {
            let paths = params
                .get("paths")
                .and_then(Value::as_array)
                .ok_or_else(|| ToolError::InvalidParams("'paths' must be an array".into()))?;

            let mut parts = Vec::new();
            for path_val in paths {
                let path = path_val
                    .as_str()
                    .ok_or_else(|| ToolError::InvalidParams("each path must be a string".into()))?;
                let target = resolve_path(&workspace_root, path)?;

                let metadata = target.metadata().map_err(|e| {
                    ToolError::ExecutionFailed(format!("cannot stat '{}': {}", path, e))
                })?;

                if metadata.len() > MAX_READ_FILE_BYTES {
                    parts.push(format!(
                        "FILE: {}\n<error: exceeds 1 MB read limit>",
                        path
                    ));
                    continue;
                }

                match fs::read_to_string(&target) {
                    Ok(content) => {
                        let content = limit_output(content, MAX_OUTPUT_BYTES);
                        parts.push(format!("FILE: {}\n{}", path, content));
                    }
                    Err(e) => {
                        parts.push(format!("FILE: {}\n<error: {}>", path, e));
                    }
                }
            }

            Ok(ToolResult {
                output: json!(parts.join("\n\n")),
                is_error: false,
            })
        })
    }
}

// ---------------------------------------------------------------------------
// SearchFilesTool
// ---------------------------------------------------------------------------

pub struct SearchFilesTool;

impl Tool for SearchFilesTool {
    fn name(&self) -> &str {
        "search_files"
    }

    fn description(&self) -> &str {
        "Search file contents using a regex pattern within the workspace."
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Low
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search for."
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in (relative to workspace root). Defaults to workspace root."
                }
            },
            "required": ["pattern"]
        })
    }

    fn execute(
        &self,
        params: Value,
        ctx: &ToolContext,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, ToolError>> + Send + '_>> {
        let workspace_root = ctx.workspace_root.clone();
        Box::pin(async move {
            let pattern = required_str(&params, "pattern")?;
            let path = params
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or(".");
            let target = resolve_path(&workspace_root, path)?;

            let re = regex::Regex::new(pattern).map_err(|e| {
                ToolError::InvalidParams(format!("invalid regex pattern: {}", e))
            })?;

            let canon_root = PathBuf::from(&workspace_root)
                .canonicalize()
                .map_err(|e| ToolError::ExecutionFailed(format!("workspace root is invalid: {}", e)))?;

            let mut matches = Vec::new();
            search_recursive(&target, &re, &canon_root, &mut matches);

            if matches.is_empty() {
                Ok(ToolResult {
                    output: json!("No matches found"),
                    is_error: false,
                })
            } else {
                let output = matches.into_iter().take(MAX_SEARCH_MATCHES).collect::<Vec<_>>().join("\n");
                let output = limit_output(output, MAX_OUTPUT_BYTES);
                Ok(ToolResult {
                    output: json!(output),
                    is_error: false,
                })
            }
        })
    }
}

fn search_recursive(
    path: &Path,
    re: &regex::Regex,
    workspace_root: &Path,
    matches: &mut Vec<String>,
) {
    if matches.len() >= MAX_SEARCH_MATCHES {
        return;
    }

    if path.is_dir() {
        let Ok(entries) = fs::read_dir(path) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if should_skip_entry(&name) {
                continue;
            }
            search_recursive(&entry.path(), re, workspace_root, matches);
            if matches.len() >= MAX_SEARCH_MATCHES {
                return;
            }
        }
        return;
    }

    // Skip files that are too large or can't be read as text.
    let Ok(metadata) = path.metadata() else {
        return;
    };
    if metadata.len() > MAX_SEARCH_FILE_BYTES {
        return;
    }

    let Ok(content) = fs::read_to_string(path) else {
        return;
    };

    for (index, line) in content.lines().enumerate() {
        if re.is_match(line) {
            let rel = path
                .strip_prefix(workspace_root)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();
            // Normalize path separators to forward slashes for consistency.
            let rel = rel.replace('\\', "/");
            matches.push(format!("{}:{}: {}", rel, index + 1, line.trim()));
            if matches.len() >= MAX_SEARCH_MATCHES {
                return;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// WriteFileTool
// ---------------------------------------------------------------------------

pub struct WriteFileTool;

impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file within the workspace, creating parent directories if needed."
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Medium
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file (relative to workspace root)."
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to the file."
                }
            },
            "required": ["path", "content"]
        })
    }

    fn execute(
        &self,
        params: Value,
        ctx: &ToolContext,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, ToolError>> + Send + '_>> {
        let workspace_root = ctx.workspace_root.clone();
        Box::pin(async move {
            let path = required_str(&params, "path")?;
            let content = required_str(&params, "content")?;
            let target = resolve_path(&workspace_root, path)?;

            // Create parent directories if they don't exist.
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    ToolError::ExecutionFailed(format!(
                        "failed to create parent directories for '{}': {}",
                        path, e
                    ))
                })?;
            }

            fs::write(&target, content).map_err(|e| {
                ToolError::ExecutionFailed(format!("failed to write '{}': {}", path, e))
            })?;

            Ok(ToolResult {
                output: json!(format!("Wrote {} bytes to {}", content.len(), path)),
                is_error: false,
            })
        })
    }
}

// ---------------------------------------------------------------------------
// EditFileTool
// ---------------------------------------------------------------------------

pub struct EditFileTool;

impl Tool for EditFileTool {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn description(&self) -> &str {
        "Replace the first occurrence of a string in a file."
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Medium
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file (relative to workspace root)."
                },
                "old_string": {
                    "type": "string",
                    "description": "The string to find and replace."
                },
                "new_string": {
                    "type": "string",
                    "description": "The replacement string."
                }
            },
            "required": ["path", "old_string", "new_string"]
        })
    }

    fn execute(
        &self,
        params: Value,
        ctx: &ToolContext,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, ToolError>> + Send + '_>> {
        let workspace_root = ctx.workspace_root.clone();
        Box::pin(async move {
            let path = required_str(&params, "path")?;
            let old_string = required_str(&params, "old_string")?;
            let new_string = required_str(&params, "new_string")?;
            let target = resolve_path(&workspace_root, path)?;

            let content = fs::read_to_string(&target).map_err(|e| {
                ToolError::ExecutionFailed(format!("failed to read '{}': {}", path, e))
            })?;

            if !content.contains(old_string) {
                return Err(ToolError::ExecutionFailed(format!(
                    "old_string not found in '{}'",
                    path
                )));
            }

            let new_content = content.replacen(old_string, new_string, 1);
            fs::write(&target, &new_content).map_err(|e| {
                ToolError::ExecutionFailed(format!("failed to write '{}': {}", path, e))
            })?;

            Ok(ToolResult {
                output: json!(format!("Edited {}", path)),
                is_error: false,
            })
        })
    }
}

// ---------------------------------------------------------------------------
// ApplyPatchTool
// ---------------------------------------------------------------------------

pub struct ApplyPatchTool;

impl Tool for ApplyPatchTool {
    fn name(&self) -> &str {
        "apply_patch"
    }

    fn description(&self) -> &str {
        "Apply a series of text replacements (edits) to a file."
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Medium
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file (relative to workspace root)."
                },
                "edits": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "old_text": { "type": "string" },
                            "new_text": { "type": "string" }
                        },
                        "required": ["old_text", "new_text"]
                    },
                    "description": "Array of {old_text, new_text} replacements to apply sequentially."
                }
            },
            "required": ["path", "edits"]
        })
    }

    fn execute(
        &self,
        params: Value,
        ctx: &ToolContext,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, ToolError>> + Send + '_>> {
        let workspace_root = ctx.workspace_root.clone();
        Box::pin(async move {
            let path = required_str(&params, "path")?;
            let target = resolve_path(&workspace_root, path)?;

            let edits = params
                .get("edits")
                .and_then(Value::as_array)
                .ok_or_else(|| ToolError::InvalidParams("'edits' must be an array".into()))?;

            let mut content = fs::read_to_string(&target).map_err(|e| {
                ToolError::ExecutionFailed(format!("failed to read '{}': {}", path, e))
            })?;

            for (i, edit) in edits.iter().enumerate() {
                let old_text = edit
                    .get("old_text")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        ToolError::InvalidParams(format!(
                            "edit[{}]: missing 'old_text' string",
                            i
                        ))
                    })?;
                let new_text = edit
                    .get("new_text")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        ToolError::InvalidParams(format!(
                            "edit[{}]: missing 'new_text' string",
                            i
                        ))
                    })?;

                if !content.contains(old_text) {
                    return Err(ToolError::ExecutionFailed(format!(
                        "edit[{}]: old_text not found in '{}'",
                        i, path
                    )));
                }

                content = content.replacen(old_text, new_text, 1);
            }

            // Write atomically via temp file.
            let temp_path = target.with_extension("eagent.tmp");
            fs::write(&temp_path, &content).map_err(|e| {
                ToolError::ExecutionFailed(format!("failed to write temp file: {}", e))
            })?;
            let _ = fs::remove_file(&target);
            fs::rename(&temp_path, &target).map_err(|e| {
                ToolError::ExecutionFailed(format!("failed to rename temp file: {}", e))
            })?;

            Ok(ToolResult {
                output: json!(format!("Patched {} ({} edits applied)", path, edits.len())),
                is_error: false,
            })
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Tool, ToolContext};
    use serde_json::json;
    use tempfile::TempDir;

    fn test_ctx(dir: &TempDir) -> ToolContext {
        ToolContext {
            workspace_root: dir.path().to_string_lossy().to_string(),
            agent_id: eagent_protocol::ids::AgentId::new(),
            task_id: eagent_protocol::ids::TaskId::new(),
            services: None,
        }
    }

    // -- resolve_path security -----------------------------------------------

    #[test]
    fn resolve_path_within_workspace() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("hello.txt");
        fs::write(&file, "hi").unwrap();
        let result = resolve_path(&dir.path().to_string_lossy(), "hello.txt");
        assert!(result.is_ok());
    }

    #[test]
    fn resolve_path_rejects_escape() {
        let dir = TempDir::new().unwrap();
        let result = resolve_path(&dir.path().to_string_lossy(), "../../etc/passwd");
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::PermissionDenied(_) => {}
            other => panic!("expected PermissionDenied, got {:?}", other),
        }
    }

    #[test]
    fn resolve_path_allows_new_file_in_existing_dir() {
        let dir = TempDir::new().unwrap();
        let result = resolve_path(&dir.path().to_string_lossy(), "new_file.txt");
        assert!(result.is_ok());
    }

    // -- ListDirectoryTool ---------------------------------------------------

    #[tokio::test]
    async fn list_directory_lists_files() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.txt"), "a").unwrap();
        fs::write(dir.path().join("b.txt"), "b").unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub").join("c.txt"), "c").unwrap();

        let tool = ListDirectoryTool;
        let ctx = test_ctx(&dir);
        let result = tool.execute(json!({}), &ctx).await.unwrap();
        let output = result.output.as_str().unwrap();
        assert!(output.contains("a.txt"));
        assert!(output.contains("b.txt"));
        assert!(output.contains("[DIR]"));
        assert!(output.contains("c.txt"));
    }

    #[tokio::test]
    async fn list_directory_skips_git() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();
        fs::write(dir.path().join(".git").join("HEAD"), "ref").unwrap();
        fs::write(dir.path().join("visible.txt"), "yes").unwrap();

        let tool = ListDirectoryTool;
        let ctx = test_ctx(&dir);
        let result = tool.execute(json!({}), &ctx).await.unwrap();
        let output = result.output.as_str().unwrap();
        assert!(output.contains("visible.txt"));
        assert!(!output.contains("HEAD"));
    }

    // -- ReadFileTool --------------------------------------------------------

    #[tokio::test]
    async fn read_file_returns_content() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("test.txt"), "hello world").unwrap();

        let tool = ReadFileTool;
        let ctx = test_ctx(&dir);
        let result = tool.execute(json!({"path": "test.txt"}), &ctx).await.unwrap();
        assert_eq!(result.output.as_str().unwrap(), "hello world");
    }

    #[tokio::test]
    async fn read_file_rejects_missing_path() {
        let dir = TempDir::new().unwrap();
        let tool = ReadFileTool;
        let ctx = test_ctx(&dir);
        let result = tool.execute(json!({}), &ctx).await;
        assert!(result.is_err());
    }

    // -- WriteFileTool -------------------------------------------------------

    #[tokio::test]
    async fn write_file_creates_file() {
        let dir = TempDir::new().unwrap();
        let tool = WriteFileTool;
        let ctx = test_ctx(&dir);
        let result = tool
            .execute(json!({"path": "new.txt", "content": "brand new"}), &ctx)
            .await
            .unwrap();
        assert!(!result.is_error);
        assert_eq!(
            fs::read_to_string(dir.path().join("new.txt")).unwrap(),
            "brand new"
        );
    }

    #[tokio::test]
    async fn write_file_creates_parent_dirs() {
        let dir = TempDir::new().unwrap();
        let tool = WriteFileTool;
        let ctx = test_ctx(&dir);
        let result = tool
            .execute(
                json!({"path": "sub/deep/file.txt", "content": "nested"}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(!result.is_error);
        assert_eq!(
            fs::read_to_string(dir.path().join("sub/deep/file.txt")).unwrap(),
            "nested"
        );
    }

    // -- EditFileTool --------------------------------------------------------

    #[tokio::test]
    async fn edit_file_replaces_text() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("doc.txt"), "hello world").unwrap();

        let tool = EditFileTool;
        let ctx = test_ctx(&dir);
        let result = tool
            .execute(
                json!({"path": "doc.txt", "old_string": "world", "new_string": "rust"}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(!result.is_error);
        assert_eq!(
            fs::read_to_string(dir.path().join("doc.txt")).unwrap(),
            "hello rust"
        );
    }

    #[tokio::test]
    async fn edit_file_fails_on_missing_old_string() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("doc.txt"), "hello world").unwrap();

        let tool = EditFileTool;
        let ctx = test_ctx(&dir);
        let result = tool
            .execute(
                json!({"path": "doc.txt", "old_string": "not here", "new_string": "x"}),
                &ctx,
            )
            .await;
        assert!(result.is_err());
    }

    // -- SearchFilesTool -----------------------------------------------------

    #[tokio::test]
    async fn search_files_finds_matches() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.rs"), "fn main() {}\nfn helper() {}").unwrap();
        fs::write(dir.path().join("b.txt"), "no match here").unwrap();

        let tool = SearchFilesTool;
        let ctx = test_ctx(&dir);
        let result = tool
            .execute(json!({"pattern": "fn \\w+"}), &ctx)
            .await
            .unwrap();
        let output = result.output.as_str().unwrap();
        assert!(output.contains("fn main"));
        assert!(output.contains("fn helper"));
    }

    #[tokio::test]
    async fn search_files_no_matches() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.txt"), "nothing relevant").unwrap();

        let tool = SearchFilesTool;
        let ctx = test_ctx(&dir);
        let result = tool
            .execute(json!({"pattern": "zzzzz"}), &ctx)
            .await
            .unwrap();
        assert_eq!(result.output.as_str().unwrap(), "No matches found");
    }

    // -- ApplyPatchTool ------------------------------------------------------

    #[tokio::test]
    async fn apply_patch_applies_edits() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("code.rs"), "let x = 1;\nlet y = 2;").unwrap();

        let tool = ApplyPatchTool;
        let ctx = test_ctx(&dir);
        let result = tool
            .execute(
                json!({
                    "path": "code.rs",
                    "edits": [
                        {"old_text": "let x = 1;", "new_text": "let x = 42;"},
                        {"old_text": "let y = 2;", "new_text": "let y = 99;"}
                    ]
                }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(!result.is_error);
        assert_eq!(
            fs::read_to_string(dir.path().join("code.rs")).unwrap(),
            "let x = 42;\nlet y = 99;"
        );
    }

    #[tokio::test]
    async fn apply_patch_fails_on_missing_old_text() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("code.rs"), "let x = 1;").unwrap();

        let tool = ApplyPatchTool;
        let ctx = test_ctx(&dir);
        let result = tool
            .execute(
                json!({
                    "path": "code.rs",
                    "edits": [{"old_text": "not here", "new_text": "x"}]
                }),
                &ctx,
            )
            .await;
        assert!(result.is_err());
    }
}
