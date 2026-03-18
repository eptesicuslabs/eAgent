//! Git manager — branch management, diffs, worktrees, and checkpointing.

use anyhow::{Context, Result};
use ecode_contracts::git::*;
use git2::{DiffOptions, Repository, StatusOptions};
use std::path::Path;
use tracing::info;

/// Manager for git operations on a repository.
pub struct GitManager {
    repo_path: String,
}

impl GitManager {
    /// Open a git repository at the given path.
    pub fn open(path: &str) -> Result<Self> {
        // Verify the path is a valid git repo
        Repository::open(path).with_context(|| format!("Failed to open git repo at {}", path))?;
        Ok(Self {
            repo_path: path.to_string(),
        })
    }

    /// Initialize a new git repository at the given path.
    pub fn init(path: &str) -> Result<Self> {
        Repository::init(path).with_context(|| format!("Failed to init git repo at {}", path))?;
        info!(%path, "Initialized new git repository");
        Ok(Self {
            repo_path: path.to_string(),
        })
    }

    /// Check if the path is inside a git repository.
    pub fn is_git_repo(path: &str) -> bool {
        Repository::open(path).is_ok()
    }

    fn repo(&self) -> Result<Repository> {
        Repository::open(&self.repo_path).context("Failed to open git repository")
    }

    /// List all branches.
    pub fn list_branches(&self) -> Result<Vec<BranchInfo>> {
        let repo = self.repo()?;
        let mut branches = Vec::new();

        for branch in repo.branches(None)? {
            let (branch, branch_type) = branch?;
            let name = branch.name()?.unwrap_or("").to_string();
            let is_head = branch.is_head();
            let is_remote = matches!(branch_type, git2::BranchType::Remote);

            let upstream = branch
                .upstream()
                .ok()
                .and_then(|u| u.name().ok().flatten().map(String::from));

            branches.push(BranchInfo {
                name,
                is_head,
                is_remote,
                upstream,
            });
        }

        Ok(branches)
    }

    /// Get the current branch name.
    pub fn current_branch(&self) -> Result<Option<String>> {
        let repo = self.repo()?;
        let head = match repo.head() {
            Ok(h) => h,
            Err(_) => return Ok(None),
        };

        if head.is_branch() {
            Ok(head.shorthand().map(String::from))
        } else {
            Ok(None)
        }
    }

    /// Create a new branch at HEAD.
    pub fn create_branch(&self, name: &str) -> Result<()> {
        let repo = self.repo()?;
        let head = repo.head()?.peel_to_commit()?;
        repo.branch(name, &head, false)?;
        info!(%name, "Created new branch");
        Ok(())
    }

    /// Checkout a branch.
    pub fn checkout(&self, branch_name: &str) -> Result<()> {
        let repo = self.repo()?;
        let refname = format!("refs/heads/{}", branch_name);
        let obj = repo.revparse_single(&refname)?;
        repo.checkout_tree(&obj, None)?;
        repo.set_head(&refname)?;
        info!(%branch_name, "Checked out branch");
        Ok(())
    }

    /// Get the status of files in the working directory.
    pub fn status(&self) -> Result<Vec<FileStatus>> {
        let repo = self.repo()?;
        let mut opts = StatusOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(true)
            .include_unmodified(false);

        let statuses = repo.statuses(Some(&mut opts))?;
        let mut files = Vec::new();

        for entry in statuses.iter() {
            let path = entry.path().unwrap_or("").to_string();
            let status = entry.status();

            let kind = if status.contains(git2::Status::WT_NEW)
                || status.contains(git2::Status::INDEX_NEW)
            {
                FileStatusKind::New
            } else if status.contains(git2::Status::WT_MODIFIED)
                || status.contains(git2::Status::INDEX_MODIFIED)
            {
                FileStatusKind::Modified
            } else if status.contains(git2::Status::WT_DELETED)
                || status.contains(git2::Status::INDEX_DELETED)
            {
                FileStatusKind::Deleted
            } else if status.contains(git2::Status::WT_RENAMED)
                || status.contains(git2::Status::INDEX_RENAMED)
            {
                FileStatusKind::Renamed
            } else if status.contains(git2::Status::WT_TYPECHANGE)
                || status.contains(git2::Status::INDEX_TYPECHANGE)
            {
                FileStatusKind::TypeChange
            } else if status.contains(git2::Status::CONFLICTED) {
                FileStatusKind::Conflicted
            } else {
                continue;
            };

            files.push(FileStatus { path, status: kind });
        }

        Ok(files)
    }

    /// Get the diff of the working directory against HEAD.
    pub fn diff_workdir(&self) -> Result<Vec<FileDiff>> {
        let repo = self.repo()?;
        let head = repo.head()?.peel_to_tree()?;
        let mut opts = DiffOptions::new();
        opts.include_untracked(true);

        let diff = repo.diff_tree_to_workdir_with_index(Some(&head), Some(&mut opts))?;
        parse_diff(&diff)
    }

    /// Get the diff between two commits.
    pub fn diff_between(&self, old_sha: &str, new_sha: &str) -> Result<Vec<FileDiff>> {
        let repo = self.repo()?;
        let old_obj = repo.revparse_single(old_sha)?;
        let new_obj = repo.revparse_single(new_sha)?;
        let old_tree = old_obj.peel_to_tree()?;
        let new_tree = new_obj.peel_to_tree()?;

        let diff = repo.diff_tree_to_tree(Some(&old_tree), Some(&new_tree), None)?;
        parse_diff(&diff)
    }

    /// Get the current HEAD commit SHA.
    pub fn head_sha(&self) -> Result<Option<String>> {
        let repo = self.repo()?;
        match repo.head() {
            Ok(head) => Ok(Some(head.peel_to_commit()?.id().to_string())),
            Err(_) => Ok(None),
        }
    }

    /// List worktrees.
    pub fn list_worktrees(&self) -> Result<Vec<WorktreeInfo>> {
        let repo = self.repo()?;
        let worktrees = repo.worktrees()?;
        let mut result = Vec::new();

        // Add main worktree
        result.push(WorktreeInfo {
            name: "main".to_string(),
            path: self.repo_path.clone(),
            branch: self.current_branch()?,
            is_main: true,
        });

        for name in worktrees.iter() {
            let name = name.unwrap_or("").to_string();
            if let Ok(wt) = repo.find_worktree(&name) {
                let path = wt.path().to_string_lossy().to_string();
                result.push(WorktreeInfo {
                    name: name.clone(),
                    path,
                    branch: None, // Would need to open the worktree's repo to get this
                    is_main: false,
                });
            }
        }

        Ok(result)
    }

    /// Create a new worktree.
    pub fn create_worktree(&self, name: &str, path: &Path, branch: Option<&str>) -> Result<()> {
        let repo = self.repo()?;

        if let Some(branch_name) = branch {
            let reference = repo.find_branch(branch_name, git2::BranchType::Local)?;
            let reference = reference.into_reference();
            repo.worktree(
                name,
                path,
                Some(git2::WorktreeAddOptions::new().reference(Some(&reference))),
            )?;
        } else {
            repo.worktree(name, path, None)?;
        }

        info!(%name, path = %path.display(), "Created worktree");
        Ok(())
    }

    /// Remove a worktree.
    pub fn remove_worktree(&self, name: &str) -> Result<()> {
        let repo = self.repo()?;
        let wt = repo.find_worktree(name)?;
        if wt.validate().is_ok() {
            wt.prune(Some(
                git2::WorktreePruneOptions::new()
                    .valid(true)
                    .working_tree(true),
            ))?;
        }
        info!(%name, "Removed worktree");
        Ok(())
    }
}

/// Parse a git2::Diff into our domain types.
fn parse_diff(diff: &git2::Diff<'_>) -> Result<Vec<FileDiff>> {
    let mut file_diffs = Vec::new();

    for delta_idx in 0..diff.deltas().len() {
        let delta = diff.get_delta(delta_idx).unwrap();
        let old_path = delta
            .old_file()
            .path()
            .map(|p| p.to_string_lossy().to_string());
        let new_path = delta
            .new_file()
            .path()
            .map(|p| p.to_string_lossy().to_string());
        let is_binary = delta.old_file().is_binary() || delta.new_file().is_binary();

        let mut hunks = Vec::new();

        if !is_binary {
            let mut patch = git2::Patch::from_diff(diff, delta_idx)?;
            if let Some(ref mut patch) = patch {
                for hunk_idx in 0..patch.num_hunks() {
                    let (hunk, num_lines) = patch.hunk(hunk_idx)?;
                    let mut lines = Vec::new();

                    for line_idx in 0..num_lines {
                        let line = patch.line_in_hunk(hunk_idx, line_idx)?;
                        let content = String::from_utf8_lossy(line.content()).to_string();
                        let kind = match line.origin() {
                            '+' => DiffLineKind::Addition,
                            '-' => DiffLineKind::Deletion,
                            _ => DiffLineKind::Context,
                        };
                        lines.push(DiffLine { kind, content });
                    }

                    hunks.push(DiffHunk {
                        old_start: hunk.old_start(),
                        old_lines: hunk.old_lines(),
                        new_start: hunk.new_start(),
                        new_lines: hunk.new_lines(),
                        lines,
                    });
                }
            }
        }

        file_diffs.push(FileDiff {
            old_path,
            new_path,
            hunks,
            is_binary,
        });
    }

    Ok(file_diffs)
}
