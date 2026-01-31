use anyhow::{Context, Result, bail};
use std::process::Stdio;
use tokio::process::Command;

use crate::file::FileNode;
use crate::snapshot::Snapshot;

#[derive(Clone)]
pub struct ResticClient
{
    repository: String,
}

impl ResticClient
{
    /// Create a client from environment variables
    pub fn from_env() -> Result<Self>
    {
        let repository = std::env::var("RESTIC_REPOSITORY")
            .context("RESTIC_REPOSITORY environment variable not set")?;

        // Verify password is available (either RESTIC_PASSWORD or RESTIC_PASSWORD_FILE)
        if std::env::var("RESTIC_PASSWORD").is_err()
            && std::env::var("RESTIC_PASSWORD_FILE").is_err()
            && std::env::var("RESTIC_PASSWORD_COMMAND").is_err()
        {
            bail!(
                "No password configured. Set RESTIC_PASSWORD, RESTIC_PASSWORD_FILE, or RESTIC_PASSWORD_COMMAND"
            );
        }

        Ok(Self { repository })
    }

    /// Build a base command with repository configured
    fn base_command(&self) -> Command
    {
        let mut cmd = Command::new("restic");
        cmd.arg("--repo").arg(&self.repository);
        cmd.arg("--json");
        cmd
    }

    /// List all snapshots in the repository
    pub async fn list_snapshots(&self) -> Result<Vec<Snapshot>>
    {
        let mut cmd = self.base_command();
        cmd.arg("snapshots");
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let output = cmd.output().await.context("Failed to run restic snapshots")?;

        if !output.status.success()
        {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("restic snapshots failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut snapshots: Vec<Snapshot> =
            serde_json::from_str(&stdout).context("Failed to parse snapshots JSON")?;

        // Sort by date descending (most recent first)
        snapshots.sort_by(|a, b| b.time.cmp(&a.time));

        Ok(snapshots)
    }

    /// List files in a snapshot at the given path
    pub async fn list_files(&self,
                            snapshot_id: &str,
                            path: &str)
                            -> Result<Vec<FileNode>>
    {
        let mut cmd = self.base_command();
        cmd.arg("ls");
        cmd.arg(snapshot_id);
        cmd.arg(path);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let output = cmd.output().await.context("Failed to run restic ls")?;

        if !output.status.success()
        {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("restic ls failed: {}", stderr);
        }

        // restic ls --json outputs one JSON object per line (NDJSON)
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut files = Vec::new();

        for line in stdout.lines()
        {
            if line.trim().is_empty()
            {
                continue;
            }

            // Parse each line as a FileNode
            match serde_json::from_str::<FileNode>(line)
            {
                Ok(node) =>
                {
                    // Skip the root entry (path == requested path)
                    // and only include direct children
                    if node.path != path && is_direct_child(&node.path, path)
                    {
                        files.push(node);
                    }
                }
                Err(_) =>
                {
                    // Skip lines that don't parse (could be status messages)
                    continue;
                }
            }
        }

        // Sort: directories first, then by name
        files.sort_by(|a, b| {
                 match (a.is_dir(), b.is_dir())
                 {
                     (true, false) => std::cmp::Ordering::Less,
                     (false, true) => std::cmp::Ordering::Greater,
                     _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                 }
             });

        Ok(files)
    }

    /// Restore a file or directory from a snapshot
    pub async fn restore(&self,
                         snapshot_id: &str,
                         include_path: &str,
                         target: &str)
                         -> Result<()>
    {
        let mut cmd = Command::new("restic");
        cmd.arg("--repo").arg(&self.repository);
        cmd.arg("restore");
        cmd.arg(snapshot_id);
        cmd.arg("--include").arg(include_path);
        cmd.arg("--target").arg(target);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let output = cmd.output().await.context("Failed to run restic restore")?;

        if !output.status.success()
        {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("restic restore failed: {}", stderr);
        }

        Ok(())
    }
}

/// Check if child_path is a direct child of parent_path
fn is_direct_child(child_path: &str,
                   parent_path: &str)
                   -> bool
{
    // Normalize paths by removing trailing slashes
    let parent = parent_path.trim_end_matches('/');
    let child = child_path.trim_end_matches('/');

    // Child must start with parent path
    if !child.starts_with(parent)
    {
        return false;
    }

    // Get the remaining part after parent
    let remaining = &child[parent.len()..];

    // Must start with / and have no other / in the remaining path
    if remaining.starts_with('/')
    {
        let after_slash = &remaining[1..];
        !after_slash.contains('/')
    }
    else if parent.is_empty() || parent == "/"
    {
        // Root case: remaining should have exactly one component
        let trimmed = remaining.trim_start_matches('/');
        !trimmed.contains('/')
    }
    else
    {
        false
    }
}
