extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub name: String,
    pub size: u64,
    pub is_directory: bool,
}

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

impl std::error::Error for FileSystemError {}

pub trait FileSystem {
    fn list_files(&mut self, path: &str) -> Result<Vec<FileInfo>, FileSystemError>;
    fn read_file(&mut self, path: &str) -> Result<String, FileSystemError>;
    fn read_file_bytes(&mut self, path: &str) -> Result<Vec<u8>, FileSystemError>;
    fn read_file_chunks(
        &mut self,
        _path: &str,
        _chunk_size: usize,
        _on_chunk: &mut dyn FnMut(&[u8]) -> Result<(), FileSystemError>,
    ) -> Result<(), FileSystemError> {
        Err(FileSystemError::NotSupported)
    }
    fn exists(&mut self, path: &str) -> bool;
    fn file_info(&mut self, path: &str) -> Result<FileInfo, FileSystemError>;

    fn scan_directory(&mut self, root: &str) -> Result<Vec<String>, FileSystemError> {
        let mut results = Vec::new();
        let mut dirs_to_scan = vec![root.to_string()];
        const SUPPORTED_EXTENSIONS: &[&str] = &[".epub", ".epu", ".txt", ".md"];
        const HIDDEN_PREFIXES: &[&str] = &[".", "System Volume Information"];

        while let Some(current_dir) = dirs_to_scan.pop() {
            let Ok(entries) = self.list_files(&current_dir) else {
                continue;
            };

            for entry in entries {
                let full_path = join_path(&current_dir, &entry.name);
                if HIDDEN_PREFIXES
                    .iter()
                    .any(|prefix| entry.name.starts_with(prefix))
                {
                    continue;
                }

                if entry.is_directory {
                    dirs_to_scan.push(full_path);
                } else {
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

        Ok(results)
    }
}

pub fn join_path(base: &str, name: &str) -> String {
    if base.ends_with('/') {
        format!("{}{}", base, name)
    } else {
        format!("{}/{}", base, name)
    }
}

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
