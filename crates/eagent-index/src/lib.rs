//! eAgent Index — project file index and codebase understanding.
//!
//! Provides a lightweight file-based project index for codebase understanding.
//! Walks the project file tree, detects languages from extensions, and generates
//! summaries suitable for injection into agent system prompts.
//!
//! Tree-sitter integration for symbol-level indexing is planned for a future version.

pub mod file_index;
pub mod project_graph;

pub use file_index::{FileEntry, FileIndex, FileKind, IndexSummary};
pub use project_graph::ProjectGraph;

use thiserror::Error;

/// Errors that can occur during indexing operations.
#[derive(Debug, Error)]
pub enum IndexError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid root path: {0}")]
    InvalidRoot(String),

    #[error("glob pattern error: {0}")]
    PatternError(#[from] glob::PatternError),
}

/// Infer a programming language name from a file extension.
///
/// Returns `None` for unrecognized extensions.
pub fn language_from_extension(ext: &str) -> Option<&'static str> {
    match ext.to_lowercase().as_str() {
        // Systems languages
        "rs" => Some("Rust"),
        "go" => Some("Go"),
        "c" => Some("C"),
        "cpp" | "cc" | "cxx" | "c++" => Some("C++"),
        "h" | "hpp" | "hxx" => Some("C/C++ Header"),
        "java" => Some("Java"),
        "cs" => Some("C#"),
        "swift" => Some("Swift"),
        "kt" | "kts" => Some("Kotlin"),

        // Scripting / dynamic
        "py" | "pyi" => Some("Python"),
        "rb" => Some("Ruby"),
        "php" => Some("PHP"),
        "lua" => Some("Lua"),
        "pl" | "pm" => Some("Perl"),
        "r" => Some("R"),
        "jl" => Some("Julia"),

        // Web / JS ecosystem
        "js" | "mjs" | "cjs" => Some("JavaScript"),
        "jsx" => Some("JavaScript"),
        "ts" | "mts" | "cts" => Some("TypeScript"),
        "tsx" => Some("TypeScript"),

        // Web markup & styles
        "html" | "htm" => Some("HTML"),
        "css" => Some("CSS"),
        "scss" | "sass" => Some("SCSS"),
        "less" => Some("Less"),
        "vue" => Some("Vue"),
        "svelte" => Some("Svelte"),

        // Data / config
        "json" | "jsonc" => Some("JSON"),
        "yaml" | "yml" => Some("YAML"),
        "toml" => Some("TOML"),
        "xml" => Some("XML"),
        "ini" | "cfg" => Some("INI"),
        "env" => Some("Env"),

        // Documentation
        "md" | "mdx" => Some("Markdown"),
        "rst" => Some("reStructuredText"),
        "tex" => Some("LaTeX"),
        "txt" => Some("Text"),

        // Shell
        "sh" | "bash" | "zsh" => Some("Shell"),
        "ps1" | "psm1" => Some("PowerShell"),
        "bat" | "cmd" => Some("Batch"),
        "fish" => Some("Fish"),

        // Other
        "sql" => Some("SQL"),
        "graphql" | "gql" => Some("GraphQL"),
        "proto" => Some("Protocol Buffers"),
        "dockerfile" => Some("Dockerfile"),
        "tf" | "hcl" => Some("HCL"),
        "nix" => Some("Nix"),
        "zig" => Some("Zig"),
        "wasm" | "wat" => Some("WebAssembly"),
        "dart" => Some("Dart"),
        "ex" | "exs" => Some("Elixir"),
        "erl" | "hrl" => Some("Erlang"),
        "hs" => Some("Haskell"),
        "ml" | "mli" => Some("OCaml"),
        "clj" | "cljs" => Some("Clojure"),
        "scala" | "sc" => Some("Scala"),

        _ => None,
    }
}

/// Default directory names to skip during indexing.
pub const DEFAULT_SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "__pycache__",
    ".next",
    "build",
    ".build",
    ".cache",
    ".venv",
    "venv",
    ".env",
    ".tox",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    "vendor",
    ".idea",
    ".vscode",
    ".vs",
    "out",
    "bin",
    "obj",
    ".gradle",
    ".dart_tool",
    ".pub-cache",
    "coverage",
    ".nyc_output",
    ".turbo",
    ".parcel-cache",
    ".svelte-kit",
    ".nuxt",
    ".output",
    "pkg",
    "bazel-out",
    "bazel-bin",
    "bazel-testlogs",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_detection_common_extensions() {
        assert_eq!(language_from_extension("rs"), Some("Rust"));
        assert_eq!(language_from_extension("ts"), Some("TypeScript"));
        assert_eq!(language_from_extension("tsx"), Some("TypeScript"));
        assert_eq!(language_from_extension("js"), Some("JavaScript"));
        assert_eq!(language_from_extension("jsx"), Some("JavaScript"));
        assert_eq!(language_from_extension("py"), Some("Python"));
        assert_eq!(language_from_extension("go"), Some("Go"));
        assert_eq!(language_from_extension("java"), Some("Java"));
        assert_eq!(language_from_extension("cpp"), Some("C++"));
        assert_eq!(language_from_extension("cc"), Some("C++"));
        assert_eq!(language_from_extension("h"), Some("C/C++ Header"));
        assert_eq!(language_from_extension("md"), Some("Markdown"));
        assert_eq!(language_from_extension("toml"), Some("TOML"));
        assert_eq!(language_from_extension("json"), Some("JSON"));
        assert_eq!(language_from_extension("yaml"), Some("YAML"));
        assert_eq!(language_from_extension("yml"), Some("YAML"));
    }

    #[test]
    fn language_detection_case_insensitive() {
        assert_eq!(language_from_extension("RS"), Some("Rust"));
        assert_eq!(language_from_extension("Py"), Some("Python"));
        assert_eq!(language_from_extension("JSON"), Some("JSON"));
    }

    #[test]
    fn language_detection_unknown() {
        assert_eq!(language_from_extension("xyz123"), None);
        assert_eq!(language_from_extension(""), None);
    }
}
