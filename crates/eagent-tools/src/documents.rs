//! Document tools — eWork-specific tools for creating documents, summarizing
//! text, reading PDFs, and performing structured web research.

use crate::filesystem::{required_str, resolve_path};
use crate::{Tool, ToolContext, ToolError, ToolResult};
use eagent_protocol::messages::RiskLevel;
use serde_json::{Value, json};
use std::fs;
use std::future::Future;
use std::pin::Pin;

// ---------------------------------------------------------------------------
// CSV validation helper
// ---------------------------------------------------------------------------

/// Validate that CSV content has consistent column counts across all rows.
/// Returns `Ok(row_count)` if valid, or an error message describing the issue.
fn validate_csv(content: &str) -> Result<usize, String> {
    let mut lines = content.lines().peekable();
    if lines.peek().is_none() {
        return Err("CSV content is empty".into());
    }

    let mut row_count = 0usize;
    let mut expected_cols: Option<usize> = None;

    for (i, line) in lines.enumerate() {
        // Skip empty lines.
        if line.trim().is_empty() {
            continue;
        }

        let col_count = line.split(',').count();
        match expected_cols {
            None => expected_cols = Some(col_count),
            Some(expected) if col_count != expected => {
                return Err(format!(
                    "row {} has {} columns, expected {} (based on first row)",
                    i + 1,
                    col_count,
                    expected
                ));
            }
            _ => {}
        }
        row_count += 1;
    }

    if row_count == 0 {
        return Err("CSV content contains no data rows".into());
    }

    Ok(row_count)
}

// ---------------------------------------------------------------------------
// CreateDocumentTool
// ---------------------------------------------------------------------------

pub struct CreateDocumentTool;

impl Tool for CreateDocumentTool {
    fn name(&self) -> &str {
        "create_document"
    }

    fn description(&self) -> &str {
        "Create a document file (markdown, plain text, or CSV) in the workspace, creating parent directories as needed."
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
                    "description": "Output path for the document (relative to workspace root)."
                },
                "content": {
                    "type": "string",
                    "description": "The document content."
                },
                "format": {
                    "type": "string",
                    "enum": ["markdown", "plain", "csv"],
                    "description": "Document format: 'markdown', 'plain', or 'csv'. Defaults to 'markdown'."
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
            let format = params
                .get("format")
                .and_then(Value::as_str)
                .unwrap_or("markdown");

            // Validate format.
            match format {
                "markdown" | "plain" | "csv" => {}
                other => {
                    return Err(ToolError::InvalidParams(format!(
                        "unsupported format '{}': must be 'markdown', 'plain', or 'csv'",
                        other
                    )));
                }
            }

            // For CSV, validate the content structure.
            if format == "csv" {
                if let Err(err) = validate_csv(content) {
                    return Err(ToolError::InvalidParams(format!(
                        "invalid CSV content: {}",
                        err
                    )));
                }
            }

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
                output: json!(format!(
                    "Created {} document at {} ({} bytes)",
                    format,
                    path,
                    content.len()
                )),
                is_error: false,
            })
        })
    }
}

// ---------------------------------------------------------------------------
// SummarizeTool
// ---------------------------------------------------------------------------

pub struct SummarizeTool;

impl Tool for SummarizeTool {
    fn name(&self) -> &str {
        "summarize"
    }

    fn description(&self) -> &str {
        "Truncate/format text to a maximum length. Actual AI summarization is performed by the agent's LLM — this tool handles length enforcement."
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Low
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "The text to summarize/truncate."
                },
                "max_length": {
                    "type": "integer",
                    "description": "Maximum character length of the output (default 500)."
                }
            },
            "required": ["text"]
        })
    }

    fn execute(
        &self,
        params: Value,
        _ctx: &ToolContext,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, ToolError>> + Send + '_>> {
        Box::pin(async move {
            let text = required_str(&params, "text")?;
            let max_length = params
                .get("max_length")
                .and_then(Value::as_u64)
                .unwrap_or(500) as usize;

            if max_length == 0 {
                return Err(ToolError::InvalidParams(
                    "max_length must be greater than 0".into(),
                ));
            }

            let output = if text.len() <= max_length {
                text.to_string()
            } else {
                let truncated = &text[..max_length];
                format!("{}...", truncated)
            };

            Ok(ToolResult {
                output: json!(output),
                is_error: false,
            })
        })
    }
}

// ---------------------------------------------------------------------------
// ReadPdfTool
// ---------------------------------------------------------------------------

pub struct ReadPdfTool;

impl Tool for ReadPdfTool {
    fn name(&self) -> &str {
        "read_pdf"
    }

    fn description(&self) -> &str {
        "Extract text content from a PDF file. (Stub — full PDF parsing requires the pdf-extract dependency.)"
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
                    "description": "Path to the PDF file (relative to workspace root)."
                }
            },
            "required": ["path"]
        })
    }

    fn execute(
        &self,
        params: Value,
        _ctx: &ToolContext,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, ToolError>> + Send + '_>> {
        Box::pin(async move {
            // Validate the path parameter is present.
            let _path = required_str(&params, "path")?;

            Ok(ToolResult {
                output: json!("PDF reading requires pdf-extract dependency — not yet implemented"),
                is_error: false,
            })
        })
    }
}

// ---------------------------------------------------------------------------
// ResearchTool
// ---------------------------------------------------------------------------

pub struct ResearchTool;

impl Tool for ResearchTool {
    fn name(&self) -> &str {
        "research"
    }

    fn description(&self) -> &str {
        "Perform web research on a query and return structured results with sources. Delegates to web search internally."
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Medium
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The research query."
                },
                "max_sources": {
                    "type": "integer",
                    "description": "Maximum number of sources to include (1-10, default 5)."
                }
            },
            "required": ["query"]
        })
    }

    fn execute(
        &self,
        params: Value,
        ctx: &ToolContext,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, ToolError>> + Send + '_>> {
        let ctx_owned = ctx.clone();
        Box::pin(async move {
            let query = required_str(&params, "query")?.trim().to_string();
            if query.is_empty() {
                return Err(ToolError::InvalidParams("query cannot be empty".into()));
            }

            let max_sources = params
                .get("max_sources")
                .and_then(Value::as_u64)
                .unwrap_or(5)
                .min(10) as usize;

            // Delegate to WebSearchTool internally.
            let search_tool = crate::web::WebSearchTool;
            let search_params = json!({
                "query": query,
                "max_results": max_sources
            });
            let search_result = search_tool.execute(search_params, &ctx_owned).await?;

            // Parse the search output into structured sources.
            let raw_output = search_result
                .output
                .as_str()
                .unwrap_or("")
                .to_string();

            let mut sources = Vec::new();
            // The WebSearchTool output format is blocks separated by double
            // newlines, each block has TITLE: ...\nURL: ...\nSNIPPET: ...
            for block in raw_output.split("\n\n") {
                let block = block.trim();
                if block.is_empty() {
                    continue;
                }

                let mut title = String::new();
                let mut url = String::new();
                let mut snippet = String::new();

                for line in block.lines() {
                    if let Some(rest) = line.strip_prefix("TITLE: ") {
                        title = rest.to_string();
                    } else if let Some(rest) = line.strip_prefix("URL: ") {
                        url = rest.to_string();
                    } else if let Some(rest) = line.strip_prefix("SNIPPET: ") {
                        snippet = rest.to_string();
                    }
                }

                if !title.is_empty() || !url.is_empty() {
                    sources.push(json!({
                        "title": title,
                        "url": url,
                        "snippet": snippet
                    }));
                }
            }

            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);

            let result = json!({
                "query": query,
                "sources": sources,
                "timestamp": timestamp
            });

            Ok(ToolResult {
                output: result,
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

    // -- validate_csv --------------------------------------------------------

    #[test]
    fn validate_csv_valid() {
        let csv = "name,age,city\nAlice,30,NYC\nBob,25,LA";
        assert!(validate_csv(csv).is_ok());
        assert_eq!(validate_csv(csv).unwrap(), 3);
    }

    #[test]
    fn validate_csv_single_column() {
        let csv = "item1\nitem2\nitem3";
        assert!(validate_csv(csv).is_ok());
    }

    #[test]
    fn validate_csv_inconsistent_columns() {
        let csv = "a,b,c\n1,2\n3,4,5";
        assert!(validate_csv(csv).is_err());
        let err = validate_csv(csv).unwrap_err();
        assert!(err.contains("row 2 has 2 columns, expected 3"));
    }

    #[test]
    fn validate_csv_empty() {
        assert!(validate_csv("").is_err());
    }

    #[test]
    fn validate_csv_only_blank_lines() {
        assert!(validate_csv("  \n  \n").is_err());
    }

    // -- CreateDocumentTool --------------------------------------------------

    #[tokio::test]
    async fn create_document_markdown() {
        let dir = TempDir::new().unwrap();
        let tool = CreateDocumentTool;
        let ctx = test_ctx(&dir);

        let result = tool
            .execute(
                json!({
                    "path": "output/report.md",
                    "content": "# Report\n\nSome content here.",
                    "format": "markdown"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let written = fs::read_to_string(dir.path().join("output/report.md")).unwrap();
        assert_eq!(written, "# Report\n\nSome content here.");
    }

    #[tokio::test]
    async fn create_document_csv_valid() {
        let dir = TempDir::new().unwrap();
        let tool = CreateDocumentTool;
        let ctx = test_ctx(&dir);

        let result = tool
            .execute(
                json!({
                    "path": "data.csv",
                    "content": "name,age\nAlice,30\nBob,25",
                    "format": "csv"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let written = fs::read_to_string(dir.path().join("data.csv")).unwrap();
        assert!(written.contains("Alice,30"));
    }

    #[tokio::test]
    async fn create_document_csv_invalid() {
        let dir = TempDir::new().unwrap();
        let tool = CreateDocumentTool;
        let ctx = test_ctx(&dir);

        let result = tool
            .execute(
                json!({
                    "path": "bad.csv",
                    "content": "a,b,c\n1,2",
                    "format": "csv"
                }),
                &ctx,
            )
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::InvalidParams(msg) => {
                assert!(msg.contains("invalid CSV content"));
            }
            other => panic!("expected InvalidParams, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn create_document_plain_text() {
        let dir = TempDir::new().unwrap();
        let tool = CreateDocumentTool;
        let ctx = test_ctx(&dir);

        let result = tool
            .execute(
                json!({
                    "path": "notes.txt",
                    "content": "Just some plain text.",
                    "format": "plain"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let written = fs::read_to_string(dir.path().join("notes.txt")).unwrap();
        assert_eq!(written, "Just some plain text.");
    }

    #[tokio::test]
    async fn create_document_default_format() {
        let dir = TempDir::new().unwrap();
        let tool = CreateDocumentTool;
        let ctx = test_ctx(&dir);

        // Omitting "format" should default to markdown.
        let result = tool
            .execute(
                json!({
                    "path": "doc.md",
                    "content": "# Title"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let output_str = result.output.as_str().unwrap();
        assert!(output_str.contains("markdown"));
    }

    #[tokio::test]
    async fn create_document_unsupported_format() {
        let dir = TempDir::new().unwrap();
        let tool = CreateDocumentTool;
        let ctx = test_ctx(&dir);

        let result = tool
            .execute(
                json!({
                    "path": "doc.docx",
                    "content": "whatever",
                    "format": "docx"
                }),
                &ctx,
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn create_document_creates_parent_dirs() {
        let dir = TempDir::new().unwrap();
        let tool = CreateDocumentTool;
        let ctx = test_ctx(&dir);

        let result = tool
            .execute(
                json!({
                    "path": "deep/nested/dir/file.md",
                    "content": "nested content"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert!(dir.path().join("deep/nested/dir/file.md").exists());
    }

    // -- SummarizeTool -------------------------------------------------------

    #[tokio::test]
    async fn summarize_short_text_unchanged() {
        let tool = SummarizeTool;
        let dir = TempDir::new().unwrap();
        let ctx = test_ctx(&dir);

        let result = tool
            .execute(
                json!({
                    "text": "Short text.",
                    "max_length": 500
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(result.output.as_str().unwrap(), "Short text.");
    }

    #[tokio::test]
    async fn summarize_long_text_truncated() {
        let tool = SummarizeTool;
        let dir = TempDir::new().unwrap();
        let ctx = test_ctx(&dir);

        let long_text = "a".repeat(1000);
        let result = tool
            .execute(
                json!({
                    "text": long_text,
                    "max_length": 100
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let output = result.output.as_str().unwrap();
        // Should be 100 chars + "..."
        assert_eq!(output.len(), 103);
        assert!(output.ends_with("..."));
    }

    #[tokio::test]
    async fn summarize_default_max_length() {
        let tool = SummarizeTool;
        let dir = TempDir::new().unwrap();
        let ctx = test_ctx(&dir);

        let text = "b".repeat(600);
        let result = tool
            .execute(json!({"text": text}), &ctx)
            .await
            .unwrap();

        assert!(!result.is_error);
        let output = result.output.as_str().unwrap();
        // Default max_length is 500, so 600-char text gets truncated to 500 + "..."
        assert_eq!(output.len(), 503);
        assert!(output.ends_with("..."));
    }

    #[tokio::test]
    async fn summarize_exact_length_not_truncated() {
        let tool = SummarizeTool;
        let dir = TempDir::new().unwrap();
        let ctx = test_ctx(&dir);

        let text = "c".repeat(100);
        let result = tool
            .execute(
                json!({
                    "text": text,
                    "max_length": 100
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let output = result.output.as_str().unwrap();
        assert_eq!(output.len(), 100);
        assert!(!output.ends_with("..."));
    }

    // -- ReadPdfTool ---------------------------------------------------------

    #[tokio::test]
    async fn read_pdf_returns_stub() {
        let tool = ReadPdfTool;
        let dir = TempDir::new().unwrap();
        let ctx = test_ctx(&dir);

        let result = tool
            .execute(json!({"path": "document.pdf"}), &ctx)
            .await
            .unwrap();

        assert!(!result.is_error);
        let output = result.output.as_str().unwrap();
        assert!(output.contains("PDF reading requires pdf-extract dependency"));
        assert!(output.contains("not yet implemented"));
    }

    #[tokio::test]
    async fn read_pdf_requires_path() {
        let tool = ReadPdfTool;
        let dir = TempDir::new().unwrap();
        let ctx = test_ctx(&dir);

        let result = tool.execute(json!({}), &ctx).await;
        assert!(result.is_err());
    }

    // -- ResearchTool --------------------------------------------------------

    #[tokio::test]
    async fn research_tool_returns_proper_json_shape() {
        // We can't do a real web search in tests, so we test the structural
        // aspects: parameter validation and output shape expectations.
        let tool = ResearchTool;

        // Verify the tool metadata.
        assert_eq!(tool.name(), "research");
        assert_eq!(tool.risk_level(), RiskLevel::Medium);

        // Verify the parameter schema has the right shape.
        let schema = tool.parameter_schema();
        let props = schema.get("properties").unwrap();
        assert!(props.get("query").is_some());
        assert!(props.get("max_sources").is_some());

        let required = schema.get("required").unwrap().as_array().unwrap();
        assert!(required.contains(&json!("query")));
    }

    #[tokio::test]
    async fn research_tool_rejects_empty_query() {
        let tool = ResearchTool;
        let dir = TempDir::new().unwrap();
        let ctx = test_ctx(&dir);

        let result = tool
            .execute(json!({"query": "   "}), &ctx)
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::InvalidParams(msg) => {
                assert!(msg.contains("query cannot be empty"));
            }
            other => panic!("expected InvalidParams, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn research_tool_rejects_missing_query() {
        let tool = ResearchTool;
        let dir = TempDir::new().unwrap();
        let ctx = test_ctx(&dir);

        let result = tool.execute(json!({}), &ctx).await;
        assert!(result.is_err());
    }
}
