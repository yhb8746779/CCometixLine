use super::{Segment, SegmentData};
use crate::config::{InputData, SegmentId};
use std::collections::HashMap;

pub struct DirectorySegment {
    show_full_path: bool,
}

impl Default for DirectorySegment {
    fn default() -> Self {
        Self::new()
    }
}

impl DirectorySegment {
    pub fn new() -> Self {
        Self { show_full_path: false }
    }

    pub fn with_full_path(mut self, show_full_path: bool) -> Self {
        self.show_full_path = show_full_path;
        self
    }

    /// Extract directory name from path, handling both Unix and Windows separators
    fn extract_directory_name(path: &str) -> String {
        // Handle Windows drive root (e.g., "D:", "D:/", "D:\")
        let trimmed = path.trim_end_matches(['/', '\\']);
        if trimmed.len() == 2 && trimmed.chars().nth(1) == Some(':') {
            return format!("{}\\", trimmed); // Return "D:\" for drive root
        }

        // Handle both Unix and Windows separators by trying both
        let unix_name = path.split('/').next_back().unwrap_or("");
        let windows_name = path.split('\\').next_back().unwrap_or("");

        // Choose the name that indicates actual path splitting occurred
        let result = if windows_name.len() < path.len() {
            // Windows path separator was found
            windows_name
        } else if unix_name.len() < path.len() {
            // Unix path separator was found
            unix_name
        } else {
            // No separator found, use the whole path
            path
        };

        if result.is_empty() {
            "root".to_string()
        } else {
            result.to_string()
        }
    }
}

impl Segment for DirectorySegment {
    fn collect(&self, input: &InputData) -> Option<SegmentData> {
        let current_dir = &input.workspace.current_dir;

        // Use full path or just directory name based on config
        let dir_name = if self.show_full_path {
            current_dir.clone()
        } else {
            Self::extract_directory_name(current_dir)
        };

        // Store the full path in metadata for potential use
        let mut metadata = HashMap::new();
        metadata.insert("full_path".to_string(), current_dir.clone());

        Some(SegmentData {
            primary: dir_name,
            secondary: String::new(),
            metadata,
        })
    }

    fn id(&self) -> SegmentId {
        SegmentId::Directory
    }
}
