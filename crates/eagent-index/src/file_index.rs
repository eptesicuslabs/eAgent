//! File tree index — walks a project directory and catalogs all files.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::{language_from_extension, IndexError, DEFAULT_SKIP_DIRS};

/// A single entry in the file index (file or directory).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    /// Path relative to the project root (using `/` separators).
    pub path: String,
    /// Whether this entry is a file or directory.
    pub kind: FileKind,
    /// Size in bytes (0 for directories).
    pub size: u64,
    /// File extension, if any.
    pub extension: Option<String>,
    /// Programming language inferred from extension.
    pub language: Option<String>,
}

/// Whether a [`FileEntry`] is a file or a directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileKind {
    File,
    Directory,
}

/// Aggregate statistics about an indexed project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexSummary {
    /// Total number of files (excluding directories).
    pub total_files: usize,
    /// Total number of directories.
    pub total_dirs: usize,
    /// Total size of all files in bytes.
    pub total_size: u64,
    /// Language breakdown: language name -> file count.
    pub languages: HashMap<String, usize>,
}

/// An in-memory index of a project's file tree.
///
/// Build the index with [`FileIndex::build`], then query with
/// [`FileIndex::search`], [`FileIndex::files_by_language`], or
/// [`FileIndex::summary`].
pub struct FileIndex {
    root: PathBuf,
    entries: Vec<FileEntry>,
    built_at: Option<DateTime<Utc>>,
    skip_dirs: Vec<String>,
}

impl FileIndex {
    /// Create a new, empty file index rooted at the given directory.
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            entries: Vec::new(),
            built_at: None,
            skip_dirs: DEFAULT_SKIP_DIRS.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Override the default skip-directory list.
    pub fn set_skip_dirs(&mut self, dirs: Vec<String>) {
        self.skip_dirs = dirs;
    }

    /// Add additional directories to skip during indexing.
    pub fn add_skip_dirs(&mut self, dirs: &[&str]) {
        for d in dirs {
            self.skip_dirs.push(d.to_string());
        }
    }

    /// The root directory of this index.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// When the index was last built, if ever.
    pub fn built_at(&self) -> Option<DateTime<Utc>> {
        self.built_at
    }

    /// Build (or rebuild) the index by walking the file tree.
    ///
    /// This replaces any previously indexed entries.
    pub fn build(&mut self) -> Result<(), IndexError> {
        if !self.root.is_dir() {
            return Err(IndexError::InvalidRoot(
                self.root.display().to_string(),
            ));
        }

        self.entries.clear();
        self.walk_dir(&self.root.clone(), &self.root.clone())?;
        self.built_at = Some(Utc::now());

        debug!(
            root = %self.root.display(),
            entries = self.entries.len(),
            "file index built"
        );

        Ok(())
    }

    /// Recursively walk directories, adding entries.
    fn walk_dir(&mut self, dir: &Path, root: &Path) -> Result<(), IndexError> {
        let mut dir_entries: Vec<fs::DirEntry> = fs::read_dir(dir)?
            .filter_map(|e| e.ok())
            .collect();

        // Sort for deterministic ordering.
        dir_entries.sort_by_key(|e| e.file_name());

        for entry in dir_entries {
            let path = entry.path();
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();

            // Skip hidden files/dirs (starting with `.`) except the ones we specifically handle.
            // The skip-dirs list already includes `.git`, etc.

            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue, // skip unreadable entries
            };

            if metadata.is_dir() {
                // Check skip list.
                if self.skip_dirs.iter().any(|s| s == name.as_ref()) {
                    continue;
                }

                let rel = self.relative_path(&path, root);
                self.entries.push(FileEntry {
                    path: rel,
                    kind: FileKind::Directory,
                    size: 0,
                    extension: None,
                    language: None,
                });

                self.walk_dir(&path, root)?;
            } else if metadata.is_file() {
                let ext = path
                    .extension()
                    .map(|e| e.to_string_lossy().to_string());
                let language = ext
                    .as_deref()
                    .and_then(language_from_extension)
                    .map(|s| s.to_string());
                let rel = self.relative_path(&path, root);

                self.entries.push(FileEntry {
                    path: rel,
                    kind: FileKind::File,
                    size: metadata.len(),
                    extension: ext,
                    language,
                });
            }
            // Symlinks and other special files are ignored.
        }

        Ok(())
    }

    /// Compute a relative path string using forward slashes.
    fn relative_path(&self, path: &Path, root: &Path) -> String {
        path.strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    }

    /// Get all indexed entries.
    pub fn entries(&self) -> &[FileEntry] {
        &self.entries
    }

    /// Search entries whose path contains the given pattern (case-insensitive substring match).
    ///
    /// For glob-style matching, the pattern supports `*` as a wildcard
    /// that matches any sequence of characters (excluding `/`).
    pub fn search(&self, pattern: &str) -> Vec<&FileEntry> {
        // Try glob-style matching first if the pattern contains wildcard chars.
        if pattern.contains('*') || pattern.contains('?') || pattern.contains('[') {
            if let Ok(glob_pat) = glob::Pattern::new(pattern) {
                return self
                    .entries
                    .iter()
                    .filter(|e| glob_pat.matches(&e.path))
                    .collect();
            }
        }

        // Fall back to case-insensitive substring matching.
        let lower = pattern.to_lowercase();
        self.entries
            .iter()
            .filter(|e| e.path.to_lowercase().contains(&lower))
            .collect()
    }

    /// Get all file entries for a given language (case-insensitive match).
    pub fn files_by_language(&self, language: &str) -> Vec<&FileEntry> {
        let lower = language.to_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                e.kind == FileKind::File
                    && e.language
                        .as_ref()
                        .is_some_and(|l| l.to_lowercase() == lower)
            })
            .collect()
    }

    /// Compute summary statistics for the indexed project.
    pub fn summary(&self) -> IndexSummary {
        let mut total_files = 0usize;
        let mut total_dirs = 0usize;
        let mut total_size = 0u64;
        let mut languages: HashMap<String, usize> = HashMap::new();

        for entry in &self.entries {
            match entry.kind {
                FileKind::File => {
                    total_files += 1;
                    total_size += entry.size;
                    if let Some(ref lang) = entry.language {
                        *languages.entry(lang.clone()).or_insert(0) += 1;
                    }
                }
                FileKind::Directory => {
                    total_dirs += 1;
                }
            }
        }

        IndexSummary {
            total_files,
            total_dirs,
            total_size,
            languages,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Helper: create a test project structure.
    fn create_test_project() -> TempDir {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        // Create directories.
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("src/utils")).unwrap();
        fs::create_dir_all(root.join("tests")).unwrap();
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::create_dir_all(root.join("node_modules/pkg")).unwrap(); // should be skipped
        fs::create_dir_all(root.join(".git/objects")).unwrap(); // should be skipped

        // Create files.
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(root.join("src/lib.rs"), "pub mod utils;").unwrap();
        fs::write(root.join("src/utils/helpers.rs"), "pub fn help() {}").unwrap();
        fs::write(root.join("tests/integration.rs"), "#[test] fn it_works() {}").unwrap();
        fs::write(root.join("docs/README.md"), "# Test Project").unwrap();
        fs::write(root.join("package.json"), "{}").unwrap();
        fs::write(root.join("src/app.tsx"), "export default () => <div/>;").unwrap();
        fs::write(root.join("node_modules/pkg/index.js"), "module.exports = {};").unwrap();

        dir
    }

    #[test]
    fn build_indexes_files_and_dirs() {
        let dir = create_test_project();
        let mut index = FileIndex::new(dir.path().to_path_buf());
        index.build().unwrap();

        assert!(index.built_at().is_some());

        let files: Vec<_> = index
            .entries()
            .iter()
            .filter(|e| e.kind == FileKind::File)
            .collect();
        let dirs: Vec<_> = index
            .entries()
            .iter()
            .filter(|e| e.kind == FileKind::Directory)
            .collect();

        // Should include our created files but not node_modules or .git contents.
        assert!(files.len() >= 6, "expected at least 6 files, got {}", files.len());
        assert!(dirs.len() >= 3, "expected at least 3 dirs, got {}", dirs.len());

        // node_modules and .git should be skipped.
        let all_paths: Vec<&str> = index.entries().iter().map(|e| e.path.as_str()).collect();
        assert!(
            !all_paths.iter().any(|p| p.contains("node_modules")),
            "node_modules should be skipped"
        );
        assert!(
            !all_paths.iter().any(|p| p.contains(".git")),
            ".git should be skipped"
        );
    }

    #[test]
    fn language_detection_in_entries() {
        let dir = create_test_project();
        let mut index = FileIndex::new(dir.path().to_path_buf());
        index.build().unwrap();

        let rust_files = index.files_by_language("Rust");
        assert!(
            rust_files.len() >= 3,
            "expected at least 3 Rust files, got {}",
            rust_files.len()
        );

        let ts_files = index.files_by_language("TypeScript");
        assert_eq!(ts_files.len(), 1, "expected 1 TypeScript file");
        assert!(ts_files[0].path.ends_with("app.tsx"));

        let md_files = index.files_by_language("Markdown");
        assert_eq!(md_files.len(), 1);
    }

    #[test]
    fn search_by_substring() {
        let dir = create_test_project();
        let mut index = FileIndex::new(dir.path().to_path_buf());
        index.build().unwrap();

        let results = index.search("helpers");
        assert_eq!(results.len(), 1);
        assert!(results[0].path.contains("helpers"));

        // Case insensitive.
        let results = index.search("HELPERS");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn search_by_glob() {
        let dir = create_test_project();
        let mut index = FileIndex::new(dir.path().to_path_buf());
        index.build().unwrap();

        let results = index.search("*.rs");
        assert!(
            results.len() >= 3,
            "expected at least 3 .rs files via glob, got {}",
            results.len()
        );

        let results = index.search("src/*.rs");
        assert!(
            results.len() >= 2,
            "expected at least 2 .rs files in src/ via glob, got {}",
            results.len()
        );
    }

    #[test]
    fn summary_statistics() {
        let dir = create_test_project();
        let mut index = FileIndex::new(dir.path().to_path_buf());
        index.build().unwrap();

        let summary = index.summary();
        assert!(summary.total_files >= 6);
        assert!(summary.total_dirs >= 3);
        assert!(summary.total_size > 0);
        assert!(summary.languages.contains_key("Rust"));
        assert!(summary.languages.contains_key("TypeScript"));
        assert!(summary.languages.contains_key("Markdown"));
        assert!(summary.languages.contains_key("JSON"));
    }

    #[test]
    fn invalid_root_returns_error() {
        let mut index = FileIndex::new(PathBuf::from("/nonexistent/path/abc123"));
        let result = index.build();
        assert!(result.is_err());
    }

    #[test]
    fn paths_use_forward_slashes() {
        let dir = create_test_project();
        let mut index = FileIndex::new(dir.path().to_path_buf());
        index.build().unwrap();

        for entry in index.entries() {
            assert!(
                !entry.path.contains('\\'),
                "path should use forward slashes: {}",
                entry.path
            );
        }
    }

    #[test]
    fn custom_skip_dirs() {
        let dir = create_test_project();
        let root = dir.path();

        // Create an extra directory that we'll configure to skip.
        fs::create_dir_all(root.join("generated")).unwrap();
        fs::write(root.join("generated/auto.rs"), "// generated").unwrap();

        let mut index = FileIndex::new(root.to_path_buf());
        index.add_skip_dirs(&["generated"]);
        index.build().unwrap();

        let all_paths: Vec<&str> = index.entries().iter().map(|e| e.path.as_str()).collect();
        assert!(
            !all_paths.iter().any(|p| p.contains("generated")),
            "generated/ should be skipped"
        );
    }
}
