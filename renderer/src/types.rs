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

/// One page within a PDF gallery
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PdfPage {
    pub url: String,
    pub width: u32,
    pub height: u32,
}

/// A gallery of PDF pages associated with a node
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PdfGallery {
    pub slug: String,
    pub page_count: u32,
    pub pages: Vec<PdfPage>,
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
    pub preview: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pdf_galleries: Option<Vec<PdfGallery>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdf_galleries_absent_when_none() {
        let entry = IndexEntry {
            id: "test-id".to_string(),
            title: "Test".to_string(),
            slug: "test".to_string(),
            aliases: vec![],
            tags: vec![],
            backlinks: vec![],
            last_modified: "2026-01-01".to_string(),
            preview: "".to_string(),
            pdf_galleries: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(!json.contains("pdf_galleries"));
    }

    #[test]
    fn pdf_galleries_present_when_some() {
        let entry = IndexEntry {
            id: "test-id".to_string(),
            title: "Test".to_string(),
            slug: "test".to_string(),
            aliases: vec![],
            tags: vec![],
            backlinks: vec![],
            last_modified: "2026-01-01".to_string(),
            preview: "".to_string(),
            pdf_galleries: Some(vec![PdfGallery {
                slug: "my-doc".to_string(),
                page_count: 1,
                pages: vec![PdfPage {
                    url: "/media/test-id/my-doc-page-1.webp".to_string(),
                    width: 800,
                    height: 1200,
                }],
            }]),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("pdf_galleries"));
        assert!(json.contains("my-doc"));
        assert!(json.contains("my-doc-page-1.webp"));
    }
}
