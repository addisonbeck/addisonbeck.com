use crate::types::NodeFile;
use std::collections::HashMap;

/// Maps node UUID -> canonical URL slug
pub type SlugMap = HashMap<String, String>;

/// Convert a string to a URL-safe slug.
/// Lowercases, replaces non-alphanumeric chars with hyphens,
/// deduplicates consecutive hyphens, trims leading/trailing hyphens.
pub fn slugify(text: &str) -> String {
    let mut slug = String::with_capacity(text.len());
    let mut prev_hyphen = false;

    for ch in text.chars() {
        if ch.is_alphanumeric() {
            slug.push(ch.to_lowercase().next().unwrap());
            prev_hyphen = false;
        } else if !prev_hyphen && !slug.is_empty() {
            slug.push('-');
            prev_hyphen = true;
        }
    }

    // Trim trailing hyphen
    if slug.ends_with('-') {
        slug.pop();
    }

    slug
}

/// Build a slug map from all nodes.
/// Maps:
///   UUID -> canonical slug (derived from title)
///   alias slugs -> UUID (for getStaticPaths alias resolution)
///   UUID string -> UUID (for direct UUID URL access)
///
/// Collision handling: if two titles produce the same slug,
/// the second gets <slug>-<first-8-chars-of-uuid>.
/// Deterministic: nodes sorted by UUID before processing.
pub fn build_slug_map(nodes: &[NodeFile]) -> SlugMap {
    let mut map: SlugMap = HashMap::new();
    let mut slug_to_uuid: HashMap<String, String> = HashMap::new();

    // Sort by UUID for deterministic collision resolution
    let mut sorted: Vec<&NodeFile> = nodes.iter().collect();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));

    for node in &sorted {
        let base_slug = slugify(&node.title);
        let canonical_slug = if slug_to_uuid.contains_key(&base_slug) {
            // Collision: append first 8 chars of UUID (lowercased)
            let suffix = node.id.replace('-', "").to_lowercase();
            let suffix_8 = &suffix[..suffix.len().min(8)];
            format!("{}-{}", base_slug, suffix_8)
        } else {
            base_slug.clone()
        };

        // Register the canonical slug
        slug_to_uuid.insert(base_slug.clone(), node.id.clone());
        // Map UUID -> canonical slug
        map.insert(node.id.clone(), canonical_slug.clone());
        // Map UUID string (for /UUID URLs) -> canonical slug
        // (Astro will also generate these paths via getStaticPaths)
    }

    map
}

/// Build a full alias resolution map: all slugs (title, aliases, UUID) -> UUID.
/// Used by Astro's getStaticPaths to generate all URL paths per node.
pub fn build_alias_map(nodes: &[NodeFile], slug_map: &SlugMap) -> HashMap<String, String> {
    let mut alias_map: HashMap<String, String> = HashMap::new();
    let mut alias_slug_to_uuid: HashMap<String, String> = HashMap::new();

    let mut sorted: Vec<&NodeFile> = nodes.iter().collect();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));

    for node in &sorted {
        // Title slug (canonical)
        if let Some(canonical) = slug_map.get(&node.id) {
            alias_map.insert(canonical.clone(), node.id.clone());
        }

        // UUID direct URL
        alias_map.insert(node.id.clone(), node.id.clone());

        // Alias slugs
        for alias in &node.aliases {
            let alias_slug = slugify(alias);
            let resolved_slug = if alias_slug_to_uuid.contains_key(&alias_slug) {
                let suffix = node.id.replace('-', "").to_lowercase();
                let suffix_8 = &suffix[..suffix.len().min(8)];
                format!("{}-{}", alias_slug, suffix_8)
            } else {
                alias_slug.clone()
            };
            alias_slug_to_uuid.insert(alias_slug, node.id.clone());
            alias_map.insert(resolved_slug, node.id.clone());
        }
    }

    alias_map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify_basic() {
        assert_eq!(slugify("Hello World"), "hello-world");
    }

    #[test]
    fn test_slugify_special_chars() {
        assert_eq!(slugify("C++: The Language"), "c-the-language");
    }

    #[test]
    fn test_slugify_leading_trailing() {
        assert_eq!(slugify(" leading and trailing "), "leading-and-trailing");
    }

    #[test]
    fn test_slugify_deduplicates_hyphens() {
        assert_eq!(slugify("a  b   c"), "a-b-c");
    }

    // REND-09: Slug collision — two nodes with identical titles
    #[test]
    fn test_slug_collision_appends_uuid_suffix() {
        let node_a = crate::types::NodeFile {
            id: "AAAAAAAA-0000-0000-0000-000000000000".to_string(),
            title: "Duplicate Title".to_string(),
            file: "".to_string(),
            point: None,
            level: None,
            tags: vec![],
            aliases: vec![],
            links_to: vec![],
            linked_from: vec![],
            ast: serde_json::Value::Null,
        };
        let node_b = crate::types::NodeFile {
            id: "BBBBBBBB-0000-0000-0000-000000000000".to_string(),
            title: "Duplicate Title".to_string(),
            file: "".to_string(),
            point: None,
            level: None,
            tags: vec![],
            aliases: vec![],
            links_to: vec![],
            linked_from: vec![],
            ast: serde_json::Value::Null,
        };
        let map = build_slug_map(&[node_a, node_b]);
        let slug_a = map.get("AAAAAAAA-0000-0000-0000-000000000000").unwrap();
        let slug_b = map.get("BBBBBBBB-0000-0000-0000-000000000000").unwrap();
        assert_ne!(slug_a, slug_b, "Colliding slugs must be different");
        // The second (by UUID sort: AAAA < BBBB, so AAAA is first) gets clean slug
        assert_eq!(slug_a, "duplicate-title");
        assert!(slug_b.starts_with("duplicate-title-"));
    }
}
