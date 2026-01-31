use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Snapshot
{
    #[serde(rename = "id")]
    pub full_id: String,
    pub short_id: String,
    pub time: DateTime<Utc>,
    pub paths: Vec<String>,
}

impl Snapshot
{
    /// Returns the display ID (short_id)
    pub fn display_id(&self) -> &str
    {
        &self.short_id
    }

    /// Returns the first path or "N/A" if no paths
    pub fn primary_path(&self) -> &str
    {
        self.paths.first().map(|s| s.as_str()).unwrap_or("N/A")
    }

    /// Formats the time for display
    pub fn formatted_time(&self) -> String
    {
        self.time.format("%Y-%m-%d %H:%M").to_string()
    }
}
