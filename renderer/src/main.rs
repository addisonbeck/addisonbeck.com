use clap::Parser;
use image::ImageReader;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::collections::HashSet;
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

    /// Force full re-render, ignoring the render cache
    #[arg(long, default_value_t = false)]
    force: bool,
}

#[derive(Debug, Default, serde::Deserialize, serde::Serialize)]
struct RenderCache {
    slug_map_hash: String,
    nodes: HashMap<String, String>,
}

const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "svg", "webp", "avif"];

/// Extensions converted to WebP. SVG and GIF are passed through unchanged.
const WEBP_CONVERT_EXTS: &[&str] = &["png", "jpg", "jpeg", "avif", "webp"];

/// Extension matched for PDF gallery processing.
const PDF_EXT: &str = "pdf";

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

/// Walk AST and collect all file: link paths that have a .pdf extension.
fn collect_pdf_paths(ast: &serde_json::Value) -> Vec<String> {
    let mut result = Vec::new();
    collect_pdf_paths_inner(ast, &mut result);
    result
}

fn collect_pdf_paths_inner(node: &serde_json::Value, out: &mut Vec<String>) {
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
                        if ext == PDF_EXT {
                            out.push(path.to_string());
                        }
                    }
                }
            }
            for child in arr.iter().skip(2) {
                collect_pdf_paths_inner(child, out);
            }
        }
    }
}

/// Invoke pdftoppm on `pdf_src`, decode each PPM page, re-encode as WebP at quality 85.0,
/// write to `media_output_dir/<node_id>/<pdf_slug>-page-<N>.webp`, return PdfGallery.
///
/// Returns Err with a diagnostic string if pdftoppm exits non-zero or any page fails.
fn convert_pdf_to_webp_pages(
    pdf_src: &str,
    node_id: &str,
    pdf_slug: &str,
    media_output_dir: &str,
) -> Result<types::PdfGallery, String> {
    use std::process::Command;

    // Stage PPM files in a uniquely-named temp subdir to avoid cross-node collisions
    let tmp_dir = std::env::temp_dir().join(format!("renderer-pdf-{}-{}", node_id, pdf_slug));
    std::fs::create_dir_all(&tmp_dir)
        .map_err(|e| format!("cannot create temp dir {:?}: {}", tmp_dir, e))?;
    let tmp_prefix = tmp_dir.join("page");
    let tmp_prefix_str = tmp_prefix.to_string_lossy().to_string();

    // Invoke pdftoppm: -scale-to 1200 scales longest side to 1200px (ADR-045)
    let status = Command::new("pdftoppm")
        .args(["-scale-to", "1200", pdf_src, &tmp_prefix_str])
        .status()
        .map_err(|e| format!("cannot spawn pdftoppm: {}", e))?;

    if !status.success() {
        let _ = std::fs::remove_dir_all(&tmp_dir);
        return Err(format!("pdftoppm exited with status {:?}", status.code()));
    }

    // Collect and sort PPM output files (pdftoppm names them page-000001.ppm, page-000002.ppm, ...)
    let mut ppm_files: Vec<std::path::PathBuf> = std::fs::read_dir(&tmp_dir)
        .map_err(|e| format!("cannot read temp dir {:?}: {}", tmp_dir, e))?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("ppm") {
                Some(path)
            } else {
                None
            }
        })
        .collect();
    ppm_files.sort();

    if ppm_files.is_empty() {
        let _ = std::fs::remove_dir_all(&tmp_dir);
        return Err("pdftoppm produced no PPM files".to_string());
    }

    // Ensure media output directory for this node exists
    let dest_dir = format!("{}/{}", media_output_dir, node_id);
    std::fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("cannot create media dest dir {}: {}", dest_dir, e))?;

    let mut pages = Vec::new();
    for (i, ppm_path) in ppm_files.iter().enumerate() {
        let page_num = i + 1;
        let webp_name = format!("{}-page-{}.webp", pdf_slug, page_num);
        let dest = format!("{}/{}", dest_dir, webp_name);
        let url = format!("/media/{}/{}", node_id, webp_name);

        let img = ImageReader::open(ppm_path)
            .and_then(|r| r.with_guessed_format())
            .map_err(|e| format!("cannot open PPM {:?}: {}", ppm_path, e))?
            .decode()
            .map_err(|e| format!("cannot decode PPM {:?}: {}", ppm_path, e))?;

        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        let webp_data = webp::Encoder::from_rgba(rgba.as_raw(), width, height).encode(85.0);
        std::fs::write(&dest, &*webp_data)
            .map_err(|e| format!("cannot write WebP {}: {}", dest, e))?;

        pages.push(types::PdfPage { url, width, height });
    }

    let _ = std::fs::remove_dir_all(&tmp_dir);

    let page_count = pages.len() as u32;
    Ok(types::PdfGallery {
        slug: pdf_slug.to_string(),
        page_count,
        pages,
    })
}

fn load_render_cache(output_dir: &str) -> RenderCache {
    let path = format!("{}/.render-cache.json", output_dir);
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn hash_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

fn hash_slug_inputs(nodes: &[types::NodeFile]) -> String {
    let mut sorted: Vec<(&str, &str, &Vec<String>)> = nodes
        .iter()
        .map(|n| (n.id.as_str(), n.title.as_str(), &n.aliases))
        .collect();
    sorted.sort_by_key(|(id, _, _)| *id);
    let repr = serde_json::to_string(
        &sorted
            .iter()
            .map(|(id, title, aliases)| {
                serde_json::json!({"id": id, "title": title, "aliases": aliases})
            })
            .collect::<Vec<_>>(),
    )
    .unwrap_or_default();
    hash_bytes(repr.as_bytes())
}

fn main() {
    let args = Args::parse();
    let input_dir = expand_tilde(&args.input);
    let syntax_set = {
        let mut builder = syntect::parsing::SyntaxSet::load_defaults_newlines().into_builder();
        let extra = concat!(env!("CARGO_MANIFEST_DIR"), "/syntaxes");
        if std::path::Path::new(extra).exists() {
            builder.add_from_folder(extra, true).unwrap_or_else(|e| {
                eprintln!("[renderer] WARNING: could not load extra syntaxes from {extra}: {e}");
            });
        }
        builder.build()
    };

    // Single-node debug mode
    if let Some(uuid) = &args.node {
        let node = load_node(&input_dir, uuid);
        let slugs = render::SlugMap::new();
        let media_map: HashMap<String, String> = HashMap::new();
        let ctx = render::RenderContext {
            slug_map: &slugs,
            media_map: &media_map,
            syntax_set: &syntax_set,
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

    // Step 4: Ensure output and media directories exist (incremental: do not wipe)
    fs::create_dir_all(output_dir)
        .unwrap_or_else(|e| panic!("Cannot create output dir {}: {}", output_dir, e));
    fs::create_dir_all(media_output_dir)
        .unwrap_or_else(|e| panic!("Cannot create media dir {}: {}", media_output_dir, e));

    // Load render cache and old index (for recovering pdf_galleries on skipped nodes)
    let mut cache = if args.force {
        RenderCache::default()
    } else {
        load_render_cache(output_dir)
    };
    let old_index: HashMap<String, types::IndexEntry> = {
        let path = format!("{}/index.json", output_dir);
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<types::IndexEntry>>(&s).ok())
            .unwrap_or_default()
            .into_iter()
            .map(|e| (e.id.clone(), e))
            .collect()
    };

    // Compute slug map hash — if it changed, all nodes must re-render
    let new_slug_map_hash = hash_slug_inputs(&nodes);
    let slug_map_changed = new_slug_map_hash != cache.slug_map_hash;
    if slug_map_changed {
        eprintln!("[renderer] slug map changed — all nodes will re-render");
    }

    // Step 5: Render each node + write index entries (incremental: skip unchanged)
    let mut index: Vec<types::IndexEntry> = Vec::new();
    let mut new_cache_nodes: HashMap<String, String> = HashMap::new();

    for node in &nodes {
        // Compute hash of source JSON file for this node
        let node_source_path = format!("{}/{}/{}.json", input_dir, &node.id[..2], node.id);
        let node_hash = fs::read(&node_source_path)
            .map(|b| hash_bytes(&b))
            .unwrap_or_default();

        // Determine whether to skip this node
        let html_exists = Path::new(&format!("{}/{}.html", output_dir, node.id)).exists();
        let cached_hash = cache.nodes.get(&node.id).map(String::as_str).unwrap_or("");
        let skip = !args.force && !slug_map_changed && node_hash == cached_hash && html_exists;

        new_cache_nodes.insert(node.id.clone(), node_hash);

        if skip {
            // Reuse old index entry (preserves pdf_galleries without re-running pdftoppm)
            if let Some(old_entry) = old_index.get(&node.id) {
                index.push(old_entry.clone());
            } else {
                // Old entry missing — fall through to full render (handled below by not skipping)
                // This branch shouldn't occur in normal operation
                eprintln!(
                    "[renderer] cache hit but no old index entry for {} — re-rendering",
                    node.id
                );
            }
            eprintln!(
                "[renderer] skipped (unchanged): {} ({})",
                node.title, node.id
            );
            continue;
        }

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

                    let src_ext = Path::new(filename)
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.to_lowercase())
                        .unwrap_or_default();

                    let (dest_filename, web_url) = if WEBP_CONVERT_EXTS.contains(&src_ext.as_str())
                    {
                        let stem = Path::new(filename)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or(filename);
                        let webp_name = format!("{stem}.webp");
                        let url = format!("/media/{}/{}", node.id, webp_name);
                        (webp_name, url)
                    } else {
                        // SVG, GIF, and unknown formats: copy verbatim
                        (
                            filename.to_string(),
                            format!("/media/{}/{}", node.id, filename),
                        )
                    };

                    let dest = format!("{}/{}", dest_dir, dest_filename);

                    if dest_filename.ends_with(".webp") && !src_ext.eq("webp") {
                        // Decode source and re-encode as WebP
                        match ImageReader::open(&src).and_then(|r| r.with_guessed_format()) {
                            Ok(reader) => match reader.decode() {
                                Ok(img) => {
                                    let rgba = img.to_rgba8();
                                    let (w, h) = rgba.dimensions();
                                    let webp_data =
                                        webp::Encoder::from_rgba(rgba.as_raw(), w, h).encode(85.0);
                                    fs::write(&dest, &*webp_data).unwrap_or_else(|e| {
                                        panic!("Cannot write WebP {}: {}", dest, e)
                                    });
                                }
                                Err(e) => {
                                    eprintln!("[renderer] WARNING: cannot decode {src}: {e}, copying verbatim");
                                    let verbatim_dest = format!("{}/{}", dest_dir, filename);
                                    fs::copy(&src, &verbatim_dest).unwrap_or_else(|e| {
                                        panic!(
                                            "Cannot copy media {} -> {}: {}",
                                            src, verbatim_dest, e
                                        )
                                    });
                                    // Use original filename in URL since WebP conversion failed
                                    media_map.insert(
                                        img_path.clone(),
                                        format!("/media/{}/{}", node.id, filename),
                                    );
                                    continue;
                                }
                            },
                            Err(e) => {
                                eprintln!(
                                    "[renderer] WARNING: cannot open {src}: {e}, copying verbatim"
                                );
                                let verbatim_dest = format!("{}/{}", dest_dir, filename);
                                fs::copy(&src, &verbatim_dest).unwrap_or_else(|e| {
                                    panic!("Cannot copy media {} -> {}: {}", src, verbatim_dest, e)
                                });
                                media_map.insert(
                                    img_path.clone(),
                                    format!("/media/{}/{}", node.id, filename),
                                );
                                continue;
                            }
                        }
                    } else {
                        // Pass through verbatim (SVG, GIF, already-WebP, unknown)
                        fs::copy(&src, &dest).unwrap_or_else(|e| {
                            panic!("Cannot copy media {} -> {}: {}", src, dest, e)
                        });
                    }

                    media_map.insert(img_path.clone(), web_url);
                }
            }
        }
        // Detect PDF attachments and convert to WebP page galleries
        let pdf_paths = collect_pdf_paths(&node.ast);
        let mut pdf_galleries: Vec<types::PdfGallery> = Vec::new();
        for pdf_path in &pdf_paths {
            if let Some(filename) = Path::new(pdf_path).file_name().and_then(|f| f.to_str()) {
                let pdf_slug = Path::new(filename)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(filename)
                    .to_string();
                let src = format!("{}/media/{}/{}", input_dir, node.id, filename);
                if Path::new(&src).exists() {
                    match convert_pdf_to_webp_pages(&src, &node.id, &pdf_slug, media_output_dir) {
                        Ok(gallery) => {
                            eprintln!(
                                "[renderer] pdf: converted {} pages for {}",
                                gallery.page_count, node.id
                            );
                            pdf_galleries.push(gallery);
                        }
                        Err(e) => {
                            eprintln!(
                                "[renderer] WARNING: pdftoppm failed for {}/{}: {}",
                                node.id, pdf_slug, e
                            );
                        }
                    }
                }
            }
        }

        let ctx = render::RenderContext {
            slug_map: &slug_map,
            media_map: &media_map,
            syntax_set: &syntax_set,
        };
        // Render HTML fragment
        let html = render::render_ast(&node.ast, &ctx);
        let html_path = format!("{}/{}.html", output_dir, node.id);
        fs::write(&html_path, &html)
            .unwrap_or_else(|e| panic!("Cannot write {}: {}", html_path, e));

        // Extract LAST_MODIFIED
        let last_modified = render::extract_last_modified(&node.ast);

        // Extract plain-text preview (first 200 chars of content)
        let preview = render::extract_preview_text(&node.ast, 200);

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
            preview,
            pdf_galleries: if pdf_galleries.is_empty() {
                None
            } else {
                Some(pdf_galleries)
            },
        });

        eprintln!("[renderer] rendered: {} ({})", node.title, node.id);
    }

    // Step 5b: Remove orphaned rendered files (nodes no longer in manifest)
    let manifest_uuids: HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();

    if let Ok(entries) = fs::read_dir(output_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("html") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if !manifest_uuids.contains(stem) {
                        fs::remove_file(&path).ok();
                        eprintln!("[renderer] removed orphaned: {}", stem);
                    }
                }
            }
        }
    }

    if let Ok(entries) = fs::read_dir(media_output_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(dir_name) = path.file_name().and_then(|s| s.to_str()) {
                    if !manifest_uuids.contains(dir_name) {
                        fs::remove_dir_all(&path).ok();
                        eprintln!("[renderer] removed orphaned media: {}", dir_name);
                    }
                }
            }
        }
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

    // Step 8: Write updated render cache
    cache.slug_map_hash = new_slug_map_hash;
    cache.nodes = new_cache_nodes;
    let cache_path = format!("{}/.render-cache.json", output_dir);
    if let Ok(cache_str) = serde_json::to_string(&cache) {
        fs::write(&cache_path, &cache_str).ok();
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_pdf_paths_finds_pdf_links() {
        let ast = serde_json::json!([
            "link",
            { "type": "file", "path": "/some/dir/document.pdf" }
        ]);
        let paths = collect_pdf_paths(&ast);
        assert_eq!(paths, vec!["/some/dir/document.pdf"]);
    }

    #[test]
    fn collect_pdf_paths_ignores_non_pdf() {
        let ast = serde_json::json!([
            "section",
            {},
            ["link", { "type": "file", "path": "/img/photo.jpg" }],
            ["link", { "type": "file", "path": "/img/icon.png" }],
            ["link", { "type": "file", "path": "/img/logo.svg" }]
        ]);
        let paths = collect_pdf_paths(&ast);
        assert!(paths.is_empty());
    }
}
