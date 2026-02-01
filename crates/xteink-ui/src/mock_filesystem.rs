//! Mock Filesystem Implementation for Simulators
//!
//! Provides a simple in-memory filesystem for testing without real hardware.

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::filesystem::{FileInfo, FileSystem, FileSystemError};

/// In-memory file entry
#[derive(Clone)]
enum MockEntry {
    File { content: String, size: u64 },
    Directory { children: Vec<String> },
}

/// Mock filesystem for simulators
///
/// Stores files in memory for testing UI without real SD card
pub struct MockFileSystem {
    files: BTreeMap<String, MockEntry>,
    current_dir: String,
}

impl MockFileSystem {
    /// Create new mock filesystem with sample files
    pub fn new() -> Self {
        let mut fs = Self {
            files: BTreeMap::new(),
            current_dir: String::from("/"),
        };

        // Create root directory first
        fs.files.insert(
            "/".to_string(),
            MockEntry::Directory {
                children: Vec::new(),
            },
        );

        // Create directory structure FIRST (before adding files)
        fs.add_directory("/books");
        fs.add_directory("/documents");

        // Add sample files for testing
        fs.add_file(
            "/books/sample.txt",
            include_str!("../../../sample_books/sample.txt"),
        );
        fs.add_file("/books/readme.txt", "Welcome to Xteink X4 e-reader!\n\nThis is a sample text file.\n\nUse the buttons to navigate:\n- Left/Right: Change page\n- Confirm: Select\n- Back: Go back\n\nEnjoy reading!");
        fs.add_file(
            "/books/war_and_peace_ch1.txt",
            include_str!("../../../sample_books/war_and_peace_ch1.txt"),
        );
        fs.add_file("/documents/notes.txt", "My reading notes:\n\n- War and Peace: 1225 pages\n- Sample book: 1 page\n\nTotal: 1226 pages read");

        fs
    }

    /// Create empty mock filesystem
    pub fn empty() -> Self {
        Self {
            files: BTreeMap::new(),
            current_dir: String::from("/"),
        }
    }

    /// Add a file to the mock filesystem
    pub fn add_file(&mut self, path: &str, content: &str) {
        let size = content.len() as u64;
        self.files.insert(
            path.to_string(),
            MockEntry::File {
                content: content.to_string(),
                size,
            },
        );

        // Add to parent directory
        let parent = crate::filesystem::dirname(path);
        if let Some(MockEntry::Directory { children }) = self.files.get_mut(parent) {
            let name = crate::filesystem::basename(path).to_string();
            if !children.contains(&name) {
                children.push(name);
            }
        }
    }

    /// Add a directory to the mock filesystem
    pub fn add_directory(&mut self, path: &str) {
        self.files.insert(
            path.to_string(),
            MockEntry::Directory {
                children: Vec::new(),
            },
        );

        // Add to parent directory
        if path != "/" {
            let parent = crate::filesystem::dirname(path);
            if let Some(MockEntry::Directory { children }) = self.files.get_mut(parent) {
                let name = crate::filesystem::basename(path).to_string();
                if !children.contains(&name) {
                    children.push(name);
                }
            }
        }
    }

    fn normalize_path(&self, path: &str) -> String {
        if path.starts_with('/') {
            path.to_string()
        } else {
            crate::filesystem::join_path(&self.current_dir, path)
        }
    }
}

impl FileSystem for MockFileSystem {
    fn list_files(&mut self, path: &str) -> Result<Vec<FileInfo>, FileSystemError> {
        let path = self.normalize_path(path);

        match self.files.get(&path) {
            Some(MockEntry::Directory { children }) => {
                let mut files = Vec::new();
                for child_name in children {
                    let child_path = crate::filesystem::join_path(&path, child_name);
                    if let Some(entry) = self.files.get(&child_path) {
                        let (size, is_directory) = match entry {
                            MockEntry::File { size, .. } => (*size, false),
                            MockEntry::Directory { .. } => (0, true),
                        };
                        files.push(FileInfo {
                            name: child_name.clone(),
                            size,
                            is_directory,
                        });
                    }
                }
                Ok(files)
            }
            Some(MockEntry::File { .. }) => {
                Err(FileSystemError::IoError("Not a directory".to_string()))
            }
            None => Err(FileSystemError::NotFound),
        }
    }

    fn read_file(&mut self, path: &str) -> Result<String, FileSystemError> {
        let path = self.normalize_path(path);

        match self.files.get(&path) {
            Some(MockEntry::File { content, .. }) => Ok(content.clone()),
            Some(MockEntry::Directory { .. }) => {
                Err(FileSystemError::IoError("Is a directory".to_string()))
            }
            None => Err(FileSystemError::NotFound),
        }
    }

    fn exists(&mut self, path: &str) -> bool {
        let path = self.normalize_path(path);
        self.files.contains_key(&path)
    }

    fn file_info(&mut self, path: &str) -> Result<FileInfo, FileSystemError> {
        let path = self.normalize_path(path);
        let name = crate::filesystem::basename(&path).to_string();

        match self.files.get(&path) {
            Some(MockEntry::File { size, .. }) => Ok(FileInfo {
                name,
                size: *size,
                is_directory: false,
            }),
            Some(MockEntry::Directory { .. }) => Ok(FileInfo {
                name,
                size: 0,
                is_directory: true,
            }),
            None => Err(FileSystemError::NotFound),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_filesystem() {
        let mut fs = MockFileSystem::new();

        // Test listing files
        let files = fs.list_files("/books").unwrap();
        assert!(!files.is_empty());

        // Test reading file
        let content = fs.read_file("/books/readme.txt").unwrap();
        assert!(content.contains("Xteink"));

        // Test file info
        let info = fs.file_info("/books/readme.txt").unwrap();
        assert!(!info.is_directory);
        assert!(info.size > 0);

        // Test exists
        assert!(fs.exists("/books"));
        assert!(!fs.exists("/nonexistent"));
    }
}
