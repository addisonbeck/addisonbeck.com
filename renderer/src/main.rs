use clap::Parser;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

mod render;
mod slug;
mod types;

#[derive(Parser, Debug)]
#[command(name = "renderer", about = "Render org-roam export to HTML fragments")]
struct Args {
    /// Path to the org-roam export directory
    #[arg(long, default_value = "~/.cache/org-roam-export")]
    input: String,

    /// Path to the rendered output directory
    #[arg(long, default_value = "rendered")]
    output: String,

    /// Render a single node UUID to stdout (debug mode, skips file output)
    #[arg(long)]
    node: Option<String>,

    /// Path to copy media files into (web public dir)
    #[arg(long, default_value = "site/public/media")]
    media_output: String,
}

const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "svg", "webp", "avif"];

/// Walk AST and collect all file: link paths that have an image extension.
fn collect_image_paths(ast: &serde_json::Value) -> Vec<String> {
    let mut result = Vec::new();
    collect_image_paths_inner(ast, &mut result);
    result
}

fn collect_image_paths_inner(node: &serde_json::Value, out: &mut Vec<String>) {
    if let Some(arr) = node.as_array() {
        if arr.len() >= 2 {
            if arr[0].as_str() == Some("link") {
                let props = &arr[1];
                if props.get("type").and_then(|v| v.as_str()) == Some("file") {
                    if let Some(path) = props.get("path").and_then(|v| v.as_str()) {
                        let ext = Path::new(path)
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.to_lowercase())
                            .unwrap_or_default();
                        if IMAGE_EXTS.contains(&ext.as_str()) {
                            out.push(path.to_string());
                        }
                    }
                }
            }
            for child in arr.iter().skip(2) {
                collect_image_paths_inner(child, out);
            }
        }
    }
}

fn main() {
    let args = Args::parse();
    let input_dir = expand_tilde(&args.input);

    // Single-node debug mode
    if let Some(uuid) = &args.node {
        let node = load_node(&input_dir, uuid);
        let slugs = render::SlugMap::new();
        let media_map: HashMap<String, String> = HashMap::new();
        let ctx = render::RenderContext {
            slug_map: &slugs,
            media_map: &media_map,
        };
        let html = render::render_ast(&node.ast, &ctx);
        println!("{html}");
        return;
    }

    // Full pipeline
    let output_dir = &args.output;
    let media_output_dir = &args.media_output;

    // Step 1: Read manifest.json
    let manifest_path = format!("{}/manifest.json", input_dir);
    let manifest_str = fs::read_to_string(&manifest_path)
        .unwrap_or_else(|e| panic!("Cannot read manifest at {}: {}", manifest_path, e));
    let manifest: Vec<serde_json::Value> = serde_json::from_str(&manifest_str)
        .unwrap_or_else(|e| panic!("Cannot parse manifest: {}", e));

    let node_ids: Vec<String> = manifest
        .iter()
        .filter_map(|v| v.get("id").and_then(|id| id.as_str()).map(String::from))
        .collect();

    eprintln!("[renderer] found {} nodes in manifest", node_ids.len());

    // Step 2: Load all nodes
    let nodes: Vec<types::NodeFile> = node_ids
        .iter()
        .map(|id| load_node(&input_dir, id))
        .collect();

    // Step 3: Build slug map
    let slug_map = slug::build_slug_map(&nodes);
    let alias_map = slug::build_alias_map(&nodes, &slug_map);

    // Step 4: Clear and recreate output directory (prevent stale files)
    if Path::new(output_dir).exists() {
        fs::remove_dir_all(output_dir)
            .unwrap_or_else(|e| panic!("Cannot clear output dir {}: {}", output_dir, e));
    }
    fs::create_dir_all(output_dir)
        .unwrap_or_else(|e| panic!("Cannot create output dir {}: {}", output_dir, e));

    // Clear and recreate media output directory
    if Path::new(media_output_dir).exists() {
        fs::remove_dir_all(media_output_dir)
            .unwrap_or_else(|e| panic!("Cannot clear media dir {}: {}", media_output_dir, e));
    }
    fs::create_dir_all(media_output_dir)
        .unwrap_or_else(|e| panic!("Cannot create media dir {}: {}", media_output_dir, e));

    // Step 5: Render each node + write index entries
    let mut index: Vec<types::IndexEntry> = Vec::new();

    for node in &nodes {
        // Build media map: scan AST for image links, copy from input media cache
        let image_paths = collect_image_paths(&node.ast);
        let mut media_map: HashMap<String, String> = HashMap::new();
        for img_path in &image_paths {
            if let Some(filename) = Path::new(img_path).file_name().and_then(|f| f.to_str()) {
                let src = format!("{}/media/{}/{}", input_dir, node.id, filename);
                if Path::new(&src).exists() {
                    let dest_dir = format!("{}/{}", media_output_dir, node.id);
                    fs::create_dir_all(&dest_dir).unwrap_or_else(|e| {
                        panic!("Cannot create media dest dir {}: {}", dest_dir, e)
                    });
                    let dest = format!("{}/{}", dest_dir, filename);
                    fs::copy(&src, &dest)
                        .unwrap_or_else(|e| panic!("Cannot copy media {} -> {}: {}", src, dest, e));
                    let web_url = format!("/media/{}/{}", node.id, filename);
                    media_map.insert(img_path.clone(), web_url);
                }
            }
        }
        let ctx = render::RenderContext {
            slug_map: &slug_map,
            media_map: &media_map,
        };
        // Render HTML fragment
        let html = render::render_ast(&node.ast, &ctx);
        let html_path = format!("{}/{}.html", output_dir, node.id);
        fs::write(&html_path, &html)
            .unwrap_or_else(|e| panic!("Cannot write {}: {}", html_path, e));

        // Extract LAST_MODIFIED
        let last_modified = render::extract_last_modified(&node.ast);

        // Get canonical slug
        let canonical_slug = slug_map
            .get(&node.id)
            .cloned()
            .unwrap_or_else(|| node.id.clone());

        index.push(types::IndexEntry {
            id: node.id.clone(),
            title: node.title.clone(),
            slug: canonical_slug,
            aliases: node.aliases.clone(),
            tags: node.tags.clone(),
            backlinks: node.linked_from.clone(),
            last_modified,
        });

        eprintln!("[renderer] rendered: {} ({})", node.title, node.id);
    }

    // Step 6: Write index.json
    let index_path = format!("{}/index.json", output_dir);
    let index_str = serde_json::to_string_pretty(&index)
        .unwrap_or_else(|e| panic!("Cannot serialize index: {}", e));
    fs::write(&index_path, &index_str).unwrap_or_else(|e| panic!("Cannot write index.json: {}", e));

    // Step 7: Write alias_map.json (consumed by Astro getStaticPaths)
    let alias_path = format!("{}/alias_map.json", output_dir);
    let alias_str = serde_json::to_string_pretty(&alias_map)
        .unwrap_or_else(|e| panic!("Cannot serialize alias_map: {}", e));
    fs::write(&alias_path, &alias_str)
        .unwrap_or_else(|e| panic!("Cannot write alias_map.json: {}", e));

    eprintln!(
        "[renderer] complete: {} nodes → {}/",
        nodes.len(),
        output_dir
    );
}

/// Load a single node file from the sharded input directory.
fn load_node(input_dir: &str, uuid: &str) -> types::NodeFile {
    let shard = &uuid[..2];
    let path = format!("{}/{}/{}.json", input_dir, shard, uuid);
    let content =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("Cannot read node {}: {}", path, e));
    serde_json::from_str(&content).unwrap_or_else(|e| panic!("Cannot parse node {}: {}", path, e))
}

/// Expand ~ to the home directory.
fn expand_tilde(path: &str) -> String {
    if path.starts_with('~') {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        path.replacen('~', &home, 1)
    } else {
        path.to_string()
    }
}
