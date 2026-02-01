use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct FileNode
{
    pub name: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub path: String,
    pub size: Option<u64>,
}

impl FileNode
{
    /// Check if this node is a directory
    pub fn is_dir(&self) -> bool
    {
        self.node_type == "dir"
    }

    /// Format size for display
    pub fn formatted_size(&self) -> String
    {
        if self.is_dir()
        {
            "[DIR]".to_string()
        }
        else
        {
            match self.size
            {
                Some(bytes) => format_bytes(bytes),
                None => "-".to_string(),
            }
        }
    }
}

/// Format bytes into human-readable size
fn format_bytes(bytes: u64) -> String
{
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB
    {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    }
    else if bytes >= MB
    {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    }
    else if bytes >= KB
    {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    }
    else
    {
        format!("{} B", bytes)
    }
}

/// Create a ".." parent directory entry
pub fn parent_entry(current_path: &str) -> FileNode
{
    let parent_path = std::path::Path::new(current_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "/".to_string());

    FileNode {
        name: "..".to_string(),
        node_type: "dir".to_string(),
        path: parent_path,
        size: None,
    }
}

/// Create a FileNode from a snapshot path (for displaying paths as directories)
pub fn path_entry(path: &str) -> FileNode
{
    FileNode {
        name: path.to_string(),
        node_type: "dir".to_string(),
        path: path.to_string(),
        size: None,
    }
}
