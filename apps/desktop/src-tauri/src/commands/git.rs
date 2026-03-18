use crate::dto::{GitCreateWorktreeInput, GitRemoveWorktreeInput, GitStatusPayload, branch_list_payload};
use ecode_contracts::git::BranchInfo;
use ecode_core::git::GitManager;
use std::path::PathBuf;

#[tauri::command]
pub fn git_status(cwd: String) -> Result<GitStatusPayload, String> {
    if !GitManager::is_git_repo(&cwd) {
        return Ok(GitStatusPayload {
            is_git_repo: false,
            current_branch: None,
            statuses: Vec::new(),
            diffs: Vec::new(),
            worktrees: Vec::new(),
        });
    }

    let git = GitManager::open(&cwd).map_err(|error| error.to_string())?;
    Ok(GitStatusPayload {
        is_git_repo: true,
        current_branch: git.current_branch().map_err(|error| error.to_string())?,
        statuses: git.status().map_err(|error| error.to_string())?,
        diffs: git.diff_workdir().map_err(|error| error.to_string())?,
        worktrees: git.list_worktrees().map_err(|error| error.to_string())?,
    })
}

#[tauri::command]
pub fn git_list_branches(cwd: String) -> Result<Vec<BranchInfo>, String> {
    let git = GitManager::open(&cwd).map_err(|error| error.to_string())?;
    Ok(branch_list_payload(
        git.list_branches().map_err(|error| error.to_string())?,
    ))
}

#[tauri::command]
pub fn git_diff_workdir(cwd: String) -> Result<GitStatusPayload, String> {
    git_status(cwd)
}

#[tauri::command]
pub fn git_create_worktree(input: GitCreateWorktreeInput) -> Result<(), String> {
    let git = GitManager::open(&input.cwd).map_err(|error| error.to_string())?;
    git.create_worktree(
        &input.name,
        &PathBuf::from(input.path),
        input.branch.as_deref(),
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn git_remove_worktree(input: GitRemoveWorktreeInput) -> Result<(), String> {
    let git = GitManager::open(&input.cwd).map_err(|error| error.to_string())?;
    git.remove_worktree(&input.name)
        .map_err(|error| error.to_string())
}
