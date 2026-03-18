//! Codex CLI version checking utilities.

use anyhow::{Context, Result, bail};
use std::process::Command;
use tracing::info;

/// Check that the codex binary exists and meets the minimum version requirement.
pub fn check_codex_version(binary_path: &str, min_version: &str) -> Result<String> {
    let output = Command::new(binary_path)
        .arg("--version")
        .output()
        .with_context(|| format!("Failed to run '{}'. Is Codex CLI installed?", binary_path))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("codex --version failed: {}", stderr.trim());
    }

    let version_str = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Extract version number (handle "codex 0.37.0" or just "0.37.0")
    let version = version_str
        .split_whitespace()
        .next_back()
        .unwrap_or(&version_str)
        .trim_start_matches('v');

    info!(version = %version, "Detected Codex CLI version");

    if !version_meets_minimum(version, min_version) {
        bail!(
            "Codex CLI version {} is too old. Minimum required: {}. Please update with: codex update",
            version,
            min_version
        );
    }

    Ok(version.to_string())
}

/// Check if a version string meets the minimum required version.
fn version_meets_minimum(version: &str, minimum: &str) -> bool {
    let parse_parts =
        |v: &str| -> Vec<u32> { v.split('.').filter_map(|p| p.parse::<u32>().ok()).collect() };

    let current = parse_parts(version);
    let required = parse_parts(minimum);

    for i in 0..required.len().max(current.len()) {
        let c = current.get(i).copied().unwrap_or(0);
        let r = required.get(i).copied().unwrap_or(0);
        if c > r {
            return true;
        }
        if c < r {
            return false;
        }
    }
    true // equal
}

/// Find the codex binary on PATH.
pub fn find_codex_binary(custom_path: Option<&str>) -> Result<String> {
    if let Some(path) = custom_path
        && !path.is_empty()
    {
        return Ok(path.to_string());
    }

    // Try to find "codex" on PATH
    let which_cmd = if cfg!(windows) { "where" } else { "which" };
    let output = Command::new(which_cmd)
        .arg("codex")
        .output()
        .context("Failed to search for codex on PATH")?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let candidates = stdout
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();
        let path = candidates
            .iter()
            .find(|path| path.ends_with(".exe") || path.ends_with(".cmd") || path.ends_with(".bat"))
            .or_else(|| candidates.iter().find(|path| !path.ends_with(".ps1")))
            .copied()
            .unwrap_or("codex")
            .to_string();
        Ok(path)
    } else {
        bail!("Codex CLI not found on PATH. Please install it: https://github.com/openai/codex")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(version_meets_minimum("0.37.0", "0.37.0"));
        assert!(version_meets_minimum("0.38.0", "0.37.0"));
        assert!(version_meets_minimum("1.0.0", "0.37.0"));
        assert!(!version_meets_minimum("0.36.9", "0.37.0"));
        assert!(!version_meets_minimum("0.36.0", "0.37.0"));
        assert!(version_meets_minimum("0.37.1", "0.37.0"));
    }
}
