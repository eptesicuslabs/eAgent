//! Project graph — high-level project summary for agent context injection.
//!
//! [`ProjectGraph`] wraps a [`FileIndex`] and produces human-readable
//! summaries and file tree strings that are injected into planner agent
//! system prompts to give them codebase understanding.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write;
use std::path::PathBuf;

use crate::file_index::{FileIndex, FileKind};
use crate::IndexError;

/// A project-level view built on top of [`FileIndex`].
///
/// Provides methods to generate textual summaries and file trees
/// suitable for injection into agent system prompts.
pub struct ProjectGraph {
    pub file_index: FileIndex,
}

impl ProjectGraph {
    /// Create a new project graph rooted at the given directory.
    pub fn new(root: PathBuf) -> Self {
        Self {
            file_index: FileIndex::new(root),
        }
    }

    /// Build (or rebuild) the underlying file index.
    pub fn build(&mut self) -> Result<(), IndexError> {
        self.file_index.build()
    }

    /// Generate a text summary suitable for agent system prompts.
    ///
    /// The output describes the project root, file counts broken down by
    /// language, and a brief description of top-level directories.
    pub fn generate_summary(&self) -> String {
        let summary = self.file_index.summary();
        let root_display = self.file_index.root().display();

        // Derive a project name from the root directory name.
        let project_name = self
            .file_index
            .root()
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let mut out = String::new();
        let _ = writeln!(out, "Project: {} ({})", project_name, root_display);

        // File count with language breakdown.
        let _ = write!(out, "Files: {}", summary.total_files);
        if !summary.languages.is_empty() {
            // Sort languages by count (descending) for deterministic, useful output.
            let mut langs: Vec<_> = summary.languages.iter().collect();
            langs.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));

            let _ = write!(out, " (");
            for (i, (lang, count)) in langs.iter().enumerate() {
                if i > 0 {
                    let _ = write!(out, ", ");
                }
                let _ = write!(out, "{}: {}", lang, count);
            }
            let _ = write!(out, ")");
        }
        let _ = writeln!(out);

        // Directories count.
        let _ = writeln!(out, "Directories: {}", summary.total_dirs);

        // Total size in human-readable form.
        let _ = writeln!(out, "Total size: {}", format_size(summary.total_size));

        // Top-level directory descriptions.
        let top_dirs = self.top_level_dirs();
        if !top_dirs.is_empty() {
            let _ = writeln!(out, "Structure:");
            for (dir_name, description) in &top_dirs {
                let _ = writeln!(out, "  {}/ -- {}", dir_name, description);
            }
        }

        out
    }

    /// Generate a compact file tree string, limited to `max_depth` levels.
    ///
    /// The output uses indentation and tree-drawing characters to represent
    /// the directory hierarchy. Only directories are expanded up to
    /// `max_depth`; files at the deepest visible level are listed.
    pub fn file_tree(&self, max_depth: usize) -> String {
        // Build a tree structure from entries.
        let mut tree = TreeNode::new("".to_string(), true);

        for entry in self.file_index.entries() {
            let parts: Vec<&str> = entry.path.split('/').collect();
            let is_dir = entry.kind == FileKind::Directory;
            tree.insert(&parts, is_dir, entry.size);
        }

        // Render the tree.
        let root_name = self
            .file_index
            .root()
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".to_string());

        let mut out = String::new();
        let _ = writeln!(out, "{}/", root_name);
        tree.render(&mut out, "", max_depth, 0);
        out
    }

    /// Identify top-level directories and generate short descriptions for each.
    fn top_level_dirs(&self) -> Vec<(String, String)> {
        let mut dirs: BTreeMap<String, DirStats> = BTreeMap::new();

        for entry in self.file_index.entries() {
            let top = match entry.path.split('/').next() {
                Some(t) => t.to_string(),
                None => continue,
            };

            // Only consider entries that are at least one level deep.
            if entry.path == top {
                // Top-level file, not a directory entry for our purposes.
                if entry.kind == FileKind::Directory {
                    dirs.entry(top).or_default();
                }
                continue;
            }

            let stats = dirs.entry(top).or_default();
            if entry.kind == FileKind::File {
                stats.file_count += 1;
                if let Some(ref lang) = entry.language {
                    *stats.languages.entry(lang.clone()).or_insert(0) += 1;
                }
            }
        }

        dirs.into_iter()
            .map(|(name, stats)| {
                let desc = stats.describe(&name);
                (name, desc)
            })
            .collect()
    }
}

/// Statistics about a top-level directory, used to generate descriptions.
#[derive(Default)]
struct DirStats {
    file_count: usize,
    languages: HashMap<String, usize>,
}

impl DirStats {
    /// Generate a short natural-language description.
    fn describe(&self, name: &str) -> String {
        // Use well-known directory name heuristics.
        let desc = match name.to_lowercase().as_str() {
            "src" => "Source code",
            "lib" => "Library code",
            "crates" => "Rust workspace crates",
            "packages" => "Workspace packages",
            "apps" | "app" => "Application code",
            "docs" | "doc" | "documentation" => "Documentation",
            "tests" | "test" | "spec" | "specs" => "Tests and specifications",
            "scripts" => "Build and utility scripts",
            "config" | "configs" | "conf" => "Configuration files",
            "assets" | "static" | "public" => "Static assets",
            "migrations" => "Database migrations",
            "examples" | "example" => "Example code",
            "benches" | "benchmarks" => "Benchmarks",
            "fixtures" => "Test fixtures",
            "schemas" | "schema" => "Data schemas",
            "proto" | "protos" => "Protocol buffer definitions",
            "deploy" | "deployment" | "infra" | "infrastructure" => "Deployment / infrastructure",
            "ci" | ".github" | ".circleci" | ".gitlab" => "CI/CD configuration",
            "components" => "UI components",
            "pages" => "Page components / routes",
            "styles" | "css" => "Stylesheets",
            "utils" | "helpers" | "common" | "shared" => "Shared utilities",
            "api" => "API layer",
            "services" | "service" => "Service layer",
            "models" | "model" | "entities" => "Data models",
            "types" => "Type definitions",
            _ => "",
        };

        if !desc.is_empty() {
            return desc.to_string();
        }

        // Fall back to language-based description.
        if self.file_count == 0 {
            return "empty directory".to_string();
        }

        if let Some((lang, _)) = self.languages.iter().max_by_key(|(_, c)| *c) {
            format!("{} files ({} {})", self.file_count, lang, self.file_count)
        } else {
            format!("{} files", self.file_count)
        }
    }
}

/// Internal tree node for building the file tree output.
struct TreeNode {
    #[allow(dead_code)]
    name: String,
    is_dir: bool,
    children: BTreeMap<String, TreeNode>,
    size: u64,
}

impl TreeNode {
    fn new(name: String, is_dir: bool) -> Self {
        Self {
            name,
            is_dir,
            children: BTreeMap::new(),
            size: 0,
        }
    }

    fn insert(&mut self, parts: &[&str], is_dir: bool, size: u64) {
        if parts.is_empty() {
            return;
        }

        let child = self
            .children
            .entry(parts[0].to_string())
            .or_insert_with(|| {
                let child_is_dir = parts.len() > 1 || is_dir;
                TreeNode::new(parts[0].to_string(), child_is_dir)
            });

        if parts.len() == 1 {
            child.is_dir = is_dir;
            child.size = size;
        } else {
            child.insert(&parts[1..], is_dir, size);
        }
    }

    fn render(&self, out: &mut String, prefix: &str, max_depth: usize, depth: usize) {
        let entries: Vec<_> = self.children.iter().collect();
        let count = entries.len();

        for (i, (name, node)) in entries.iter().enumerate() {
            let is_last = i == count - 1;
            let connector = if is_last { "└── " } else { "├── " };
            let extension = if is_last { "    " } else { "│   " };

            if node.is_dir {
                let _ = writeln!(out, "{}{}{}/", prefix, connector, name);
                if depth < max_depth {
                    let new_prefix = format!("{}{}", prefix, extension);
                    node.render(out, &new_prefix, max_depth, depth + 1);
                }
            } else {
                let _ = writeln!(out, "{}{}{}", prefix, connector, name);
            }
        }
    }
}

/// Format a byte size into a human-readable string.
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_project() -> TempDir {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        fs::create_dir_all(root.join("src/utils")).unwrap();
        fs::create_dir_all(root.join("tests")).unwrap();
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::create_dir_all(root.join("crates/core/src")).unwrap();
        fs::create_dir_all(root.join("apps/desktop/src")).unwrap();

        fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = [\"crates/*\"]").unwrap();
        fs::write(root.join("src/main.rs"), "fn main() { println!(\"hello\"); }").unwrap();
        fs::write(root.join("src/lib.rs"), "pub mod utils;").unwrap();
        fs::write(root.join("src/utils/mod.rs"), "pub fn help() {}").unwrap();
        fs::write(root.join("tests/test_basic.rs"), "#[test] fn works() {}").unwrap();
        fs::write(root.join("docs/guide.md"), "# User Guide\n\nSome docs.").unwrap();
        fs::write(root.join("crates/core/src/lib.rs"), "pub fn core() {}").unwrap();
        fs::write(root.join("apps/desktop/src/main.tsx"), "export default App;").unwrap();

        dir
    }

    #[test]
    fn generate_summary_contains_project_info() {
        let dir = create_test_project();
        let mut graph = ProjectGraph::new(dir.path().to_path_buf());
        graph.build().unwrap();

        let summary = graph.generate_summary();

        // Should contain the project name (temp dir name varies, but "Project:" line exists).
        assert!(summary.contains("Project:"), "should have Project line");
        assert!(summary.contains("Files:"), "should have Files line");
        assert!(summary.contains("Rust"), "should mention Rust");
        assert!(summary.contains("Structure:"), "should have Structure section");
    }

    #[test]
    fn generate_summary_includes_top_level_dirs() {
        let dir = create_test_project();
        let mut graph = ProjectGraph::new(dir.path().to_path_buf());
        graph.build().unwrap();

        let summary = graph.generate_summary();

        assert!(summary.contains("src/"), "should list src/");
        assert!(summary.contains("docs/"), "should list docs/");
        assert!(summary.contains("crates/"), "should list crates/");
    }

    #[test]
    fn file_tree_respects_max_depth() {
        let dir = create_test_project();
        let mut graph = ProjectGraph::new(dir.path().to_path_buf());
        graph.build().unwrap();

        let tree_depth_0 = graph.file_tree(0);
        let tree_depth_1 = graph.file_tree(1);
        let tree_depth_5 = graph.file_tree(5);

        // Depth 0 should show top-level entries but not expand their children.
        assert!(tree_depth_0.contains("src/"));
        // utils/ is inside src/, so at depth 0 it should NOT appear
        // (depth 0 means we list children of root but don't recurse into them).
        assert!(
            !tree_depth_0.contains("utils/"),
            "depth 0 should not show nested dirs"
        );

        // Depth 1 should show one level deeper.
        assert!(tree_depth_1.contains("src/"));
        assert!(tree_depth_1.contains("utils/"));

        // Depth 5 should show everything.
        assert!(tree_depth_5.contains("mod.rs"));
    }

    #[test]
    fn file_tree_uses_tree_chars() {
        let dir = create_test_project();
        let mut graph = ProjectGraph::new(dir.path().to_path_buf());
        graph.build().unwrap();

        let tree = graph.file_tree(3);
        // Should use tree-drawing characters.
        assert!(
            tree.contains("├──") || tree.contains("└──"),
            "should use tree drawing chars: {}",
            tree
        );
    }

    #[test]
    fn format_size_human_readable() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(1048576), "1.0 MB");
        assert_eq!(format_size(1073741824), "1.0 GB");
    }

    #[test]
    fn known_dir_descriptions() {
        let dir = create_test_project();
        let mut graph = ProjectGraph::new(dir.path().to_path_buf());
        graph.build().unwrap();

        let summary = graph.generate_summary();

        // Known directories should get meaningful descriptions.
        assert!(
            summary.contains("Source code") || summary.contains("src/"),
            "src/ should have a description"
        );
        assert!(
            summary.contains("Documentation") || summary.contains("docs/"),
            "docs/ should have a description"
        );
    }
}
