//! Attachment store — manage image and file attachments for Codex inputs.

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tracing::info;

/// Store for managing file attachments (images, etc.).
pub struct AttachmentStore {
    base_dir: PathBuf,
}

impl AttachmentStore {
    /// Create a new attachment store at the given directory.
    pub fn new(base_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&base_dir)?;
        Ok(Self { base_dir })
    }

    /// Create an attachment store in the default location.
    pub fn default_store() -> Result<Self> {
        let dir = crate::config::ConfigManager::attachments_dir()?;
        Self::new(dir)
    }

    /// Save an attachment from raw bytes. Returns the SHA-256 hash as the key.
    pub fn save(&self, data: &[u8], extension: &str) -> Result<String> {
        let hash = Self::hash_bytes(data);
        let filename = format!("{}.{}", hash, extension);
        let path = self.base_dir.join(&filename);

        if !path.exists() {
            std::fs::write(&path, data)
                .with_context(|| format!("Failed to write attachment {}", filename))?;
            info!(%hash, %extension, "Saved attachment");
        }

        Ok(hash)
    }

    /// Save an attachment from a file path. Returns the SHA-256 hash.
    pub fn save_file(&self, source: &Path) -> Result<String> {
        let data = std::fs::read(source)
            .with_context(|| format!("Failed to read file {}", source.display()))?;
        let extension = source.extension().and_then(|e| e.to_str()).unwrap_or("bin");
        self.save(&data, extension)
    }

    /// Load an attachment by its hash.
    pub fn load(&self, hash: &str) -> Result<Option<Vec<u8>>> {
        // Search for any file starting with the hash
        let entries = std::fs::read_dir(&self.base_dir)?;
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(hash) {
                let data = std::fs::read(entry.path())?;
                return Ok(Some(data));
            }
        }
        Ok(None)
    }

    /// Get the filesystem path for an attachment.
    pub fn path_for(&self, hash: &str) -> Option<PathBuf> {
        let entries = std::fs::read_dir(&self.base_dir).ok()?;
        for entry in entries {
            let entry = entry.ok()?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(hash) {
                return Some(entry.path());
            }
        }
        None
    }

    /// Delete an attachment by its hash.
    pub fn delete(&self, hash: &str) -> Result<bool> {
        if let Some(path) = self.path_for(hash) {
            std::fs::remove_file(&path)?;
            info!(%hash, "Deleted attachment");
            return Ok(true);
        }
        Ok(false)
    }

    /// List all stored attachments (hash, extension, size).
    pub fn list(&self) -> Result<Vec<AttachmentInfo>> {
        let mut result = Vec::new();
        let entries = std::fs::read_dir(&self.base_dir)?;

        for entry in entries {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy().to_string();

            if let Some((hash, ext)) = name_str.rsplit_once('.') {
                let metadata = entry.metadata()?;
                result.push(AttachmentInfo {
                    hash: hash.to_string(),
                    extension: ext.to_string(),
                    size: metadata.len(),
                });
            }
        }

        Ok(result)
    }

    /// Detect the MIME type from file extension.
    pub fn mime_type(extension: &str) -> &'static str {
        match extension.to_lowercase().as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "svg" => "image/svg+xml",
            "pdf" => "application/pdf",
            "txt" => "text/plain",
            "json" => "application/json",
            _ => "application/octet-stream",
        }
    }

    /// Hash bytes using SHA-256.
    fn hash_bytes(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    /// Get total storage usage in bytes.
    pub fn total_size(&self) -> Result<u64> {
        let mut total = 0u64;
        let entries = std::fs::read_dir(&self.base_dir)?;
        for entry in entries {
            let entry = entry?;
            total += entry.metadata()?.len();
        }
        Ok(total)
    }
}

/// Information about a stored attachment.
#[derive(Debug, Clone)]
pub struct AttachmentInfo {
    pub hash: String,
    pub extension: String,
    pub size: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let store = AttachmentStore::new(dir.path().to_path_buf()).unwrap();

        let data = b"hello world";
        let hash = store.save(data, "txt").unwrap();
        assert!(!hash.is_empty());

        let loaded = store.load(&hash).unwrap().unwrap();
        assert_eq!(loaded, data);
    }

    #[test]
    fn test_dedup() {
        let dir = tempfile::tempdir().unwrap();
        let store = AttachmentStore::new(dir.path().to_path_buf()).unwrap();

        let data = b"duplicate data";
        let h1 = store.save(data, "txt").unwrap();
        let h2 = store.save(data, "txt").unwrap();
        assert_eq!(h1, h2);

        let files: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn test_list_and_delete() {
        let dir = tempfile::tempdir().unwrap();
        let store = AttachmentStore::new(dir.path().to_path_buf()).unwrap();

        let h1 = store.save(b"file1", "txt").unwrap();
        let h2 = store.save(b"file2", "png").unwrap();

        let list = store.list().unwrap();
        assert_eq!(list.len(), 2);

        store.delete(&h1).unwrap();
        let list = store.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].hash, h2);
    }
}
