//! Git tools — status, diff, commit, and branch operations.

use crate::{Tool, ToolContext, ToolError, ToolResult};
use eagent_protocol::messages::RiskLevel;
use git2::{DiffOptions, Repository, Signature, StatusOptions};
use serde_json::{Value, json};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract a required string field from a JSON value.
fn required_str<'a>(params: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    params
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::InvalidParams(format!("missing required string parameter '{}'", key)))
}

/// Open the git repository at the workspace root.
fn open_repo(workspace_root: &str) -> Result<Repository, ToolError> {
    Repository::open(workspace_root)
        .map_err(|e| ToolError::ExecutionFailed(format!("failed to open git repository: {}", e)))
}

/// Map a git2 status to a human-readable string.
fn status_to_string(status: git2::Status) -> &'static str {
    if status.contains(git2::Status::WT_NEW) || status.contains(git2::Status::INDEX_NEW) {
        "new"
    } else if status.contains(git2::Status::WT_MODIFIED)
        || status.contains(git2::Status::INDEX_MODIFIED)
    {
        "modified"
    } else if status.contains(git2::Status::WT_DELETED)
        || status.contains(git2::Status::INDEX_DELETED)
    {
        "deleted"
    } else if status.contains(git2::Status::WT_RENAMED)
        || status.contains(git2::Status::INDEX_RENAMED)
    {
        "renamed"
    } else if status.contains(git2::Status::WT_TYPECHANGE)
        || status.contains(git2::Status::INDEX_TYPECHANGE)
    {
        "typechange"
    } else if status.contains(git2::Status::CONFLICTED) {
        "conflicted"
    } else {
        "unknown"
    }
}

// ---------------------------------------------------------------------------
// GitStatusTool
// ---------------------------------------------------------------------------

pub struct GitStatusTool;

impl Tool for GitStatusTool {
    fn name(&self) -> &str {
        "git_status"
    }

    fn description(&self) -> &str {
        "Get the git status of the working directory, returning changed files as JSON."
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Low
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    fn execute(
        &self,
        _params: Value,
        ctx: &ToolContext,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, ToolError>> + Send + '_>> {
        let workspace_root = ctx.workspace_root.clone();
        Box::pin(async move {
            let repo = open_repo(&workspace_root)?;

            let mut opts = StatusOptions::new();
            opts.include_untracked(true)
                .recurse_untracked_dirs(true)
                .include_unmodified(false);

            let statuses = repo
                .statuses(Some(&mut opts))
                .map_err(|e| ToolError::ExecutionFailed(format!("failed to get status: {}", e)))?;

            let mut files = Vec::new();
            for entry in statuses.iter() {
                let path = entry.path().unwrap_or("").to_string();
                let status = entry.status();
                let kind = status_to_string(status);

                // Determine staging state
                let staged = status.intersects(
                    git2::Status::INDEX_NEW
                        | git2::Status::INDEX_MODIFIED
                        | git2::Status::INDEX_DELETED
                        | git2::Status::INDEX_RENAMED
                        | git2::Status::INDEX_TYPECHANGE,
                );

                files.push(json!({
                    "path": path,
                    "status": kind,
                    "staged": staged,
                }));
            }

            // Get current branch
            let branch = repo
                .head()
                .ok()
                .and_then(|h| h.shorthand().map(String::from));

            let output = json!({
                "branch": branch,
                "files": files,
                "total_changes": files.len(),
            });

            Ok(ToolResult {
                output,
                is_error: false,
            })
        })
    }
}

// ---------------------------------------------------------------------------
// GitDiffTool
// ---------------------------------------------------------------------------

pub struct GitDiffTool;

impl Tool for GitDiffTool {
    fn name(&self) -> &str {
        "git_diff"
    }

    fn description(&self) -> &str {
        "Get the diff of working directory changes against HEAD."
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Low
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "staged": {
                    "type": "boolean",
                    "description": "If true, show only staged changes. Default: false (show all changes)."
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
            let staged_only = params
                .get("staged")
                .and_then(Value::as_bool)
                .unwrap_or(false);

            let repo = open_repo(&workspace_root)?;

            let head_tree = repo
                .head()
                .ok()
                .and_then(|h| h.peel_to_tree().ok());

            let mut opts = DiffOptions::new();
            opts.include_untracked(true);

            let diff = if staged_only {
                // Staged changes: diff between HEAD tree and index
                repo.diff_tree_to_index(
                    head_tree.as_ref(),
                    None,
                    Some(&mut opts),
                )
            } else {
                // All changes: diff between HEAD tree and workdir (including index)
                repo.diff_tree_to_workdir_with_index(
                    head_tree.as_ref(),
                    Some(&mut opts),
                )
            }
            .map_err(|e| ToolError::ExecutionFailed(format!("failed to compute diff: {}", e)))?;

            let mut output = String::new();
            diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
                let prefix = match line.origin() {
                    '+' => "+",
                    '-' => "-",
                    ' ' => " ",
                    'F' => "", // file header
                    'H' => "", // hunk header
                    _ => "",
                };
                let content = String::from_utf8_lossy(line.content());

                // For file and hunk headers, the content already includes necessary info
                if matches!(line.origin(), 'F' | 'H') {
                    output.push_str(&content);
                } else {
                    output.push_str(prefix);
                    output.push_str(&content);
                }
                true
            })
            .map_err(|e| ToolError::ExecutionFailed(format!("failed to format diff: {}", e)))?;

            if output.is_empty() {
                Ok(ToolResult {
                    output: json!("No changes detected"),
                    is_error: false,
                })
            } else {
                // Truncate very large diffs
                if output.len() > 64_000 {
                    output.truncate(64_000);
                    output.push_str("\n...<truncated>");
                }
                Ok(ToolResult {
                    output: json!(output),
                    is_error: false,
                })
            }
        })
    }
}

// ---------------------------------------------------------------------------
// GitCommitTool
// ---------------------------------------------------------------------------

pub struct GitCommitTool;

impl Tool for GitCommitTool {
    fn name(&self) -> &str {
        "git_commit"
    }

    fn description(&self) -> &str {
        "Stage specified files (or all changes) and create a git commit with a message."
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Medium
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "The commit message."
                },
                "files": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Files to stage before committing. If omitted, commits whatever is currently staged."
                }
            },
            "required": ["message"]
        })
    }

    fn execute(
        &self,
        params: Value,
        ctx: &ToolContext,
    ) -> Pin<Box<dyn Future<Output = Result<ToolResult, ToolError>> + Send + '_>> {
        let workspace_root = ctx.workspace_root.clone();
        Box::pin(async move {
            let message = required_str(&params, "message")?;

            let repo = open_repo(&workspace_root)?;

            // Stage files if specified
            if let Some(files) = params.get("files").and_then(Value::as_array) {
                let mut index = repo
                    .index()
                    .map_err(|e| ToolError::ExecutionFailed(format!("failed to get index: {}", e)))?;

                for file_val in files {
                    let file_path = file_val
                        .as_str()
                        .ok_or_else(|| ToolError::InvalidParams("each file must be a string".into()))?;

                    let path = Path::new(file_path);

                    // Check if the file was deleted — if so, remove from index
                    let full_path = Path::new(&workspace_root).join(path);
                    if full_path.exists() {
                        index.add_path(path).map_err(|e| {
                            ToolError::ExecutionFailed(format!(
                                "failed to stage '{}': {}",
                                file_path, e
                            ))
                        })?;
                    } else {
                        index.remove_path(path).map_err(|e| {
                            ToolError::ExecutionFailed(format!(
                                "failed to remove '{}' from index: {}",
                                file_path, e
                            ))
                        })?;
                    }
                }

                index.write().map_err(|e| {
                    ToolError::ExecutionFailed(format!("failed to write index: {}", e))
                })?;
            }

            // Read the index to get the tree
            let mut index = repo
                .index()
                .map_err(|e| ToolError::ExecutionFailed(format!("failed to get index: {}", e)))?;

            let tree_oid = index
                .write_tree()
                .map_err(|e| ToolError::ExecutionFailed(format!("failed to write tree: {}", e)))?;

            let tree = repo
                .find_tree(tree_oid)
                .map_err(|e| ToolError::ExecutionFailed(format!("failed to find tree: {}", e)))?;

            // Build signature
            let sig = repo
                .signature()
                .or_else(|_| Signature::now("eAgent", "eagent@ecode.dev"))
                .map_err(|e| ToolError::ExecutionFailed(format!("failed to create signature: {}", e)))?;

            // Get parent commit (if any)
            let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
            let parents: Vec<&git2::Commit<'_>> = parent.iter().collect();

            let commit_oid = repo
                .commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
                .map_err(|e| ToolError::ExecutionFailed(format!("failed to create commit: {}", e)))?;

            Ok(ToolResult {
                output: json!({
                    "sha": commit_oid.to_string(),
                    "message": message,
                }),
                is_error: false,
            })
        })
    }
}

// ---------------------------------------------------------------------------
// GitBranchTool
// ---------------------------------------------------------------------------

pub struct GitBranchTool;

impl Tool for GitBranchTool {
    fn name(&self) -> &str {
        "git_branch"
    }

    fn description(&self) -> &str {
        "List, create, or switch git branches."
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Medium
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "create", "switch"],
                    "description": "The action to perform. Default: 'list'."
                },
                "name": {
                    "type": "string",
                    "description": "Branch name (required for 'create' and 'switch' actions)."
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
            let action = params
                .get("action")
                .and_then(Value::as_str)
                .unwrap_or("list");

            let repo = open_repo(&workspace_root)?;

            match action {
                "list" => {
                    let branches = repo
                        .branches(None)
                        .map_err(|e| ToolError::ExecutionFailed(format!("failed to list branches: {}", e)))?;

                    let mut branch_list = Vec::new();
                    for branch in branches {
                        let (branch, branch_type) = branch.map_err(|e| {
                            ToolError::ExecutionFailed(format!("failed to read branch: {}", e))
                        })?;
                        let name = branch
                            .name()
                            .map_err(|e| ToolError::ExecutionFailed(format!("invalid branch name: {}", e)))?
                            .unwrap_or("")
                            .to_string();
                        let is_head = branch.is_head();
                        let is_remote = matches!(branch_type, git2::BranchType::Remote);

                        branch_list.push(json!({
                            "name": name,
                            "is_head": is_head,
                            "is_remote": is_remote,
                        }));
                    }

                    Ok(ToolResult {
                        output: json!({
                            "branches": branch_list,
                        }),
                        is_error: false,
                    })
                }

                "create" => {
                    let name = required_str(&params, "name")?;
                    let head = repo
                        .head()
                        .map_err(|e| ToolError::ExecutionFailed(format!("no HEAD commit: {}", e)))?
                        .peel_to_commit()
                        .map_err(|e| ToolError::ExecutionFailed(format!("HEAD is not a commit: {}", e)))?;

                    repo.branch(name, &head, false).map_err(|e| {
                        ToolError::ExecutionFailed(format!("failed to create branch '{}': {}", name, e))
                    })?;

                    Ok(ToolResult {
                        output: json!(format!("Created branch '{}'", name)),
                        is_error: false,
                    })
                }

                "switch" => {
                    let name = required_str(&params, "name")?;
                    let refname = format!("refs/heads/{}", name);
                    let obj = repo
                        .revparse_single(&refname)
                        .map_err(|e| ToolError::ExecutionFailed(format!("branch '{}' not found: {}", name, e)))?;

                    repo.checkout_tree(&obj, None).map_err(|e| {
                        ToolError::ExecutionFailed(format!("failed to checkout '{}': {}", name, e))
                    })?;

                    repo.set_head(&refname).map_err(|e| {
                        ToolError::ExecutionFailed(format!("failed to set HEAD to '{}': {}", name, e))
                    })?;

                    Ok(ToolResult {
                        output: json!(format!("Switched to branch '{}'", name)),
                        is_error: false,
                    })
                }

                other => Err(ToolError::InvalidParams(format!(
                    "unknown action '{}': expected 'list', 'create', or 'switch'",
                    other
                ))),
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
    use crate::ToolContext;
    use tempfile::TempDir;

    fn test_ctx(dir: &TempDir) -> ToolContext {
        ToolContext {
            workspace_root: dir.path().to_string_lossy().to_string(),
            agent_id: eagent_protocol::ids::AgentId::new(),
            task_id: eagent_protocol::ids::TaskId::new(),
            services: None,
        }
    }

    /// Initialize a git repo with an initial commit so HEAD exists.
    fn init_repo_with_commit(dir: &TempDir) -> Repository {
        let repo = Repository::init(dir.path()).unwrap();

        // Create an initial file and commit
        std::fs::write(dir.path().join("README.md"), "# Test\n").unwrap();
        {
            let mut index = repo.index().unwrap();
            index.add_path(Path::new("README.md")).unwrap();
            index.write().unwrap();
            let tree_oid = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_oid).unwrap();
            let sig = Signature::now("Test", "test@test.com").unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "initial commit", &tree, &[])
                .unwrap();
        }

        repo
    }

    // -- GitStatusTool --------------------------------------------------------

    #[tokio::test]
    async fn git_status_shows_changes() {
        let dir = TempDir::new().unwrap();
        let _repo = init_repo_with_commit(&dir);

        // Create a new untracked file
        std::fs::write(dir.path().join("new_file.txt"), "hello").unwrap();

        let tool = GitStatusTool;
        let ctx = test_ctx(&dir);
        let result = tool.execute(json!({}), &ctx).await.unwrap();
        assert!(!result.is_error);

        let files = result.output["files"].as_array().unwrap();
        assert!(!files.is_empty());

        // Find the new file in the results
        let new_file = files
            .iter()
            .find(|f| f["path"].as_str().unwrap() == "new_file.txt")
            .expect("new_file.txt should be in status");
        assert_eq!(new_file["status"].as_str().unwrap(), "new");
    }

    #[tokio::test]
    async fn git_status_shows_branch() {
        let dir = TempDir::new().unwrap();
        let _repo = init_repo_with_commit(&dir);

        let tool = GitStatusTool;
        let ctx = test_ctx(&dir);
        let result = tool.execute(json!({}), &ctx).await.unwrap();
        assert!(!result.is_error);

        // Should have a branch field
        assert!(result.output["branch"].is_string());
    }

    #[tokio::test]
    async fn git_status_fails_on_non_repo() {
        let dir = TempDir::new().unwrap();
        let tool = GitStatusTool;
        let ctx = test_ctx(&dir);
        let result = tool.execute(json!({}), &ctx).await;
        assert!(result.is_err());
    }

    // -- GitDiffTool ----------------------------------------------------------

    #[tokio::test]
    async fn git_diff_shows_changes() {
        let dir = TempDir::new().unwrap();
        let _repo = init_repo_with_commit(&dir);

        // Modify the README
        std::fs::write(dir.path().join("README.md"), "# Modified\n").unwrap();

        let tool = GitDiffTool;
        let ctx = test_ctx(&dir);
        let result = tool.execute(json!({}), &ctx).await.unwrap();
        assert!(!result.is_error);

        let output = result.output.as_str().unwrap();
        assert!(output.contains("Modified"));
    }

    #[tokio::test]
    async fn git_diff_no_changes() {
        let dir = TempDir::new().unwrap();
        let _repo = init_repo_with_commit(&dir);

        let tool = GitDiffTool;
        let ctx = test_ctx(&dir);
        let result = tool.execute(json!({}), &ctx).await.unwrap();
        assert!(!result.is_error);
        assert_eq!(result.output.as_str().unwrap(), "No changes detected");
    }

    // -- GitCommitTool --------------------------------------------------------

    #[tokio::test]
    async fn git_commit_creates_commit() {
        let dir = TempDir::new().unwrap();
        let _repo = init_repo_with_commit(&dir);

        // Create a new file
        std::fs::write(dir.path().join("new.txt"), "content").unwrap();

        let tool = GitCommitTool;
        let ctx = test_ctx(&dir);
        let result = tool
            .execute(
                json!({
                    "message": "add new file",
                    "files": ["new.txt"]
                }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.output["sha"].is_string());
        assert_eq!(result.output["message"].as_str().unwrap(), "add new file");

        // Verify the commit was created
        let repo = Repository::open(dir.path()).unwrap();
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.message().unwrap(), "add new file");
    }

    #[tokio::test]
    async fn git_commit_fails_without_message() {
        let dir = TempDir::new().unwrap();
        let _repo = init_repo_with_commit(&dir);

        let tool = GitCommitTool;
        let ctx = test_ctx(&dir);
        let result = tool.execute(json!({}), &ctx).await;
        assert!(result.is_err());
    }

    // -- GitBranchTool --------------------------------------------------------

    #[tokio::test]
    async fn git_branch_list() {
        let dir = TempDir::new().unwrap();
        let _repo = init_repo_with_commit(&dir);

        let tool = GitBranchTool;
        let ctx = test_ctx(&dir);
        let result = tool.execute(json!({"action": "list"}), &ctx).await.unwrap();
        assert!(!result.is_error);

        let branches = result.output["branches"].as_array().unwrap();
        assert!(!branches.is_empty());
    }

    #[tokio::test]
    async fn git_branch_create_and_switch() {
        let dir = TempDir::new().unwrap();
        let _repo = init_repo_with_commit(&dir);

        let tool = GitBranchTool;
        let ctx = test_ctx(&dir);

        // Create a new branch
        let result = tool
            .execute(json!({"action": "create", "name": "feature-x"}), &ctx)
            .await
            .unwrap();
        assert!(!result.is_error);

        // Switch to the new branch
        let result = tool
            .execute(json!({"action": "switch", "name": "feature-x"}), &ctx)
            .await
            .unwrap();
        assert!(!result.is_error);

        // Verify we're on the new branch
        let repo = Repository::open(dir.path()).unwrap();
        let head = repo.head().unwrap();
        assert_eq!(head.shorthand().unwrap(), "feature-x");
    }

    #[tokio::test]
    async fn git_branch_create_fails_duplicate() {
        let dir = TempDir::new().unwrap();
        let _repo = init_repo_with_commit(&dir);

        let tool = GitBranchTool;
        let ctx = test_ctx(&dir);

        // The default branch exists, trying to create it again should fail
        // First let's find the current branch name
        let repo = Repository::open(dir.path()).unwrap();
        let head = repo.head().unwrap();
        let branch_name = head.shorthand().unwrap().to_string();

        let result = tool
            .execute(json!({"action": "create", "name": branch_name}), &ctx)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn git_branch_switch_nonexistent() {
        let dir = TempDir::new().unwrap();
        let _repo = init_repo_with_commit(&dir);

        let tool = GitBranchTool;
        let ctx = test_ctx(&dir);
        let result = tool
            .execute(json!({"action": "switch", "name": "does-not-exist"}), &ctx)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn git_branch_unknown_action() {
        let dir = TempDir::new().unwrap();
        let _repo = init_repo_with_commit(&dir);

        let tool = GitBranchTool;
        let ctx = test_ctx(&dir);
        let result = tool
            .execute(json!({"action": "delete"}), &ctx)
            .await;
        assert!(result.is_err());
    }
}
