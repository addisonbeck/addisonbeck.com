use serde::{Deserialize, Serialize};

/// A node entry from the org-roam export manifest.json
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NodeFile {
    pub id: String,
    pub title: String,
    pub file: String,
    pub point: Option<serde_json::Value>,
    pub level: Option<u64>,
    pub tags: Vec<String>,
    pub aliases: Vec<String>,
    pub links_to: Vec<String>,
    pub linked_from: Vec<String>,
    pub ast: serde_json::Value,
}

/// An entry in the rendered/index.json output manifest
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct IndexEntry {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub aliases: Vec<String>,
    pub tags: Vec<String>,
    pub backlinks: Vec<String>,
    pub last_modified: String,
}
