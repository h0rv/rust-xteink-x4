//! Filesystem abstraction for e-reader.
//! Supports SD card on embedded devices and mock filesystem for simulators.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

/// A file entry in the filesystem
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub name: String,
    pub size: u64,
    pub is_directory: bool,
}

/// Filesystem error types
#[derive(Debug, Clone)]
pub enum FileSystemError {
    NotFound,
    PermissionDenied,
    IoError(String),
    NotSupported,
}

impl core::fmt::Display for FileSystemError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            FileSystemError::NotFound => write!(f, "File not found"),
            FileSystemError::PermissionDenied => write!(f, "Permission denied"),
            FileSystemError::IoError(msg) => write!(f, "IO error: {}", msg),
            FileSystemError::NotSupported => write!(f, "Operation not supported"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for FileSystemError {}

/// Trait for filesystem operations
///
/// Implementations:
/// - `SdCardFileSystem` for embedded (real SD card)
/// - `MockFileSystem` for simulators
pub trait FileSystem {
    /// List files in a directory
    fn list_files(&mut self, path: &str) -> Result<Vec<FileInfo>, FileSystemError>;

    /// Read entire file as string
    ///
    /// # Arguments
    /// * `path` - Path to file (e.g., "/books/book.txt")
    ///
    /// # Returns
    /// File contents as string
    ///
    /// # Errors
    /// Returns FileSystemError if file not found or read fails
    fn read_file(&mut self, path: &str) -> Result<String, FileSystemError>;

    /// Read entire file as bytes
    ///
    /// # Arguments
    /// * `path` - Path to file (e.g., "/books/book.epub")
    ///
    /// # Returns
    /// File contents as bytes
    ///
    /// # Errors
    /// Returns FileSystemError if file not found or read fails
    fn read_file_bytes(&mut self, path: &str) -> Result<Vec<u8>, FileSystemError>;

    /// Check if file exists
    fn exists(&mut self, path: &str) -> bool;

    /// Get file info
    fn file_info(&mut self, path: &str) -> Result<FileInfo, FileSystemError>;

    /// Scan directory recursively for book files
    ///
    /// # Arguments
    /// * `root` - Root directory to scan (e.g., "/books")
    ///
    /// # Returns
    /// List of file paths for supported book formats (.epub, .txt, .md)
    fn scan_directory(&mut self, root: &str) -> Result<Vec<String>, FileSystemError> {
        let mut results = Vec::new();
        let mut dirs_to_scan = vec![root.to_string()];
        const SUPPORTED_EXTENSIONS: &[&str] = &[".epub", ".txt", ".md"];
        const HIDDEN_PREFIXES: &[&str] = &[".", "System Volume Information"];

        while let Some(current_dir) = dirs_to_scan.pop() {
            match self.list_files(&current_dir) {
                Ok(entries) => {
                    for entry in entries {
                        let full_path = join_path(&current_dir, &entry.name);

                        // Skip hidden files and system directories
                        if HIDDEN_PREFIXES
                            .iter()
                            .any(|prefix| entry.name.starts_with(prefix))
                        {
                            continue;
                        }

                        if entry.is_directory {
                            dirs_to_scan.push(full_path);
                        } else {
                            // Check if it's a supported book format
                            let name_lower = entry.name.to_lowercase();
                            if SUPPORTED_EXTENSIONS
                                .iter()
                                .any(|ext| name_lower.ends_with(ext))
                            {
                                results.push(full_path);
                            }
                        }
                    }
                }
                Err(_) => continue, // Skip directories we can't read
            }
        }

        Ok(results)
    }
}

/// Filter files by extension
pub fn filter_by_extension(files: &[FileInfo], extensions: &[&str]) -> Vec<FileInfo> {
    files
        .iter()
        .filter(|f| {
            if f.is_directory {
                return true; // Keep directories
            }
            let name_lower = f.name.to_lowercase();
            extensions
                .iter()
                .any(|ext| name_lower.ends_with(&ext.to_lowercase()))
        })
        .cloned()
        .collect()
}

/// Get filename without path
pub fn basename(path: &str) -> &str {
    path.rfind('/').map(|i| &path[i + 1..]).unwrap_or(path)
}

/// Get parent directory
pub fn dirname(path: &str) -> &str {
    match path.rfind('/') {
        Some(0) => "/",
        Some(i) => &path[..i],
        None => ".",
    }
}

/// Join paths
pub fn join_path(base: &str, name: &str) -> String {
    if base.ends_with('/') {
        format!("{}{}", base, name)
    } else {
        format!("{}/{}", base, name)
    }
}

/// Resolve a logical path against a mount prefix
pub fn resolve_mount_path(path: &str, mount_prefix: &str) -> String {
    let prefix = mount_prefix.trim_end_matches('/');
    if path.starts_with(prefix) {
        path.to_string()
    } else if path.starts_with('/') {
        format!("{}{}", prefix, path)
    } else {
        format!("{}/{}", prefix, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basename() {
        assert_eq!(basename("/books/book.txt"), "book.txt");
        assert_eq!(basename("book.txt"), "book.txt");
        assert_eq!(basename("/"), "");
    }

    #[test]
    fn test_dirname() {
        assert_eq!(dirname("/books/book.txt"), "/books");
        assert_eq!(dirname("/book.txt"), "/");
        assert_eq!(dirname("book.txt"), ".");
    }

    #[test]
    fn test_join_path() {
        assert_eq!(join_path("/books", "book.txt"), "/books/book.txt");
        assert_eq!(join_path("/books/", "book.txt"), "/books/book.txt");
    }

    #[test]
    fn test_resolve_mount_path() {
        assert_eq!(
            resolve_mount_path("/books/book.txt", "/sd"),
            "/sd/books/book.txt"
        );
        assert_eq!(
            resolve_mount_path("books/book.txt", "/sd"),
            "/sd/books/book.txt"
        );
        assert_eq!(
            resolve_mount_path("/sd/books/book.txt", "/sd"),
            "/sd/books/book.txt"
        );
        assert_eq!(
            resolve_mount_path("/books/book.txt", "/sd/"),
            "/sd/books/book.txt"
        );
    }
}
