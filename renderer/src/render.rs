use serde_json::Value;
use std::collections::HashMap;
use syntect::html::{ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// Maps node UUID -> canonical URL slug
pub type SlugMap = HashMap<String, String>;

/// Context passed through the render tree.
pub struct RenderContext<'a> {
    pub slug_map: &'a SlugMap,
    pub media_map: &'a HashMap<String, String>,
    pub syntax_set: &'a SyntaxSet,
}

/// Escape HTML special characters
pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Render a complete node AST to an HTML fragment string.
/// `ast` is the full AST value from NodeFile.ast.
/// `ctx` carries slug and media maps for resolving links.
pub fn render_ast(ast: &Value, ctx: &RenderContext) -> String {
    render_node(ast, ctx)
}

/// Recursively render a single AST node (array or string) to HTML.
fn render_node(node: &Value, ctx: &RenderContext) -> String {
    match node {
        // Plain text string — HTML-escape it
        Value::String(s) => html_escape(s),

        // Element array: ["type", props_or_null, ...children]
        Value::Array(arr) if arr.len() >= 2 => {
            let element_type = arr[0].as_str().unwrap_or("");
            let props = &arr[1];
            let children = &arr[2..];
            let html = dispatch(element_type, props, children, ctx);
            // org-element stores trailing whitespace after inline elements in
            // post-blank (e.g. the space between *bold* and the next word).
            // Append it so inline runs don't collapse into each other.
            let post_blank = props
                .as_object()
                .and_then(|obj| obj.get("post-blank"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            if post_blank > 0 {
                format!("{}{}", html, " ".repeat(post_blank))
            } else {
                html
            }
        }

        // Unexpected value type — skip
        _ => String::new(),
    }
}

/// Render all children and join without separator.
fn render_children(children: &[Value], ctx: &RenderContext) -> String {
    children.iter().map(|c| render_node(c, ctx)).collect()
}

/// Dispatch on element type string and produce HTML.
fn dispatch(element_type: &str, props: &Value, children: &[Value], ctx: &RenderContext) -> String {
    match element_type {
        // --- Transparent containers: render children, no wrapper ---
        "org-data" | "section" => render_children(children, ctx),

        // --- HTML-wrapped containers ---
        "paragraph" => {
            let inner = render_children(children, ctx);
            format!("<p>{}</p>\n", inner.trim_end())
        }
        "bold" => format!("<strong>{}</strong>", render_children(children, ctx)),
        "italic" => format!("<em>{}</em>", render_children(children, ctx)),
        "underline" => format!("<u>{}</u>", render_children(children, ctx)),
        "strike-through" => format!("<del>{}</del>", render_children(children, ctx)),
        "quote-block" => format!(
            "<blockquote>\n{}\n</blockquote>\n",
            render_children(children, ctx)
        ),
        "center-block" => format!(
            "<div class=\"center\">{}\n</div>\n",
            render_children(children, ctx)
        ),
        "verse-block" => format!(
            "<pre class=\"verse\">{}</pre>\n",
            render_children(children, ctx)
        ),
        "plain-list" => {
            let tag = match props.get("type").and_then(|v| v.as_str()) {
                Some("ordered") => "ol",
                _ => "ul",
            };
            format!("<{tag}>\n{}</{tag}>\n", render_children(children, ctx))
        }
        "item" => {
            let checkbox = props.get("checkbox").and_then(|v| v.as_str());
            let checkbox_html = match checkbox {
                Some("on") => "<input type=\"checkbox\" checked disabled> ",
                Some("off") => "<input type=\"checkbox\" disabled> ",
                Some("trans") => "<input type=\"checkbox\" disabled> ",
                _ => "",
            };
            format!(
                "<li>{}{}</li>\n",
                checkbox_html,
                render_children(children, ctx)
            )
        }
        "table" => format!("<table>\n{}</table>\n", render_children(children, ctx)),
        "table-row" => match props.get("type").and_then(|v| v.as_str()) {
            Some("rule") => String::new(),
            _ => format!("<tr>{}\n</tr>\n", render_children(children, ctx)),
        },
        "table-cell" => format!("<td>{}</td>", render_children(children, ctx)),

        // --- Headline: level-based h1-h6 ---
        "headline" => render_headline(props, children, ctx),

        // --- Self-contained: content in props.value ---
        "verbatim" | "code" => {
            let value = props.get("value").and_then(|v| v.as_str()).unwrap_or("");
            format!("<code>{}</code>", html_escape(value))
        }
        "src-block" => {
            let lang = props.get("language").and_then(|v| v.as_str()).unwrap_or("");
            let value = props.get("value").and_then(|v| v.as_str()).unwrap_or("");
            let value = value.trim_end_matches('\n');
            highlight_src_block(lang, value, ctx.syntax_set)
        }
        "example-block" | "fixed-width" => {
            let value = props.get("value").and_then(|v| v.as_str()).unwrap_or("");
            format!("<pre><code>{}</code></pre>\n", html_escape(value))
        }

        // --- Special rendering ---
        "link" => render_link(props, children, ctx),
        "timestamp" => {
            let raw = props
                .get("raw-value")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            format!("<time>{}</time>", html_escape(raw))
        }
        "subscript" => format!("_{}", render_children(children, ctx)),
        "superscript" => format!("^{}", render_children(children, ctx)),
        "entity" => {
            // Use HTML entity if available, fall back to name.
            // Only pass through values that look like proper HTML entities
            // (&foo; or &#123;) to prevent XSS via poisoned export cache.
            let raw = props
                .get("html")
                .and_then(|v| v.as_str())
                .or_else(|| props.get("name").and_then(|v| v.as_str()))
                .unwrap_or("");
            if raw.starts_with('&') && raw.ends_with(';') {
                raw.to_string()
            } else {
                html_escape(raw)
            }
        }
        "line-break" => "<br>\n".to_string(),
        "horizontal-rule" => "<hr>\n".to_string(),

        // --- Metadata: skip entirely ---
        "property-drawer" | "node-property" | "keyword" | "planning" | "comment"
        | "comment-block" | "clock" | "drawer" | "babel-call" | "diary-sexp" | "dynamic-block"
        | "export-block" | "export-snippet" | "inline-babel-call" | "macro" | "radio-target"
        | "target" | "special-block" | "inlinetask" | "statistics-cookie" | "todo-keyword" => {
            String::new()
        }

        // --- Catch-all: render children transparently ---
        unknown => {
            eprintln!("[renderer] unknown element type: {unknown}");
            render_children(children, ctx)
        }
    }
}

/// Render a headline element.
/// Level is clamped to 1-6. Title is the `props.title` array rendered recursively.
/// todo-keyword is always null in exported AST (Emacs export strips it);
/// TODO/DONE state appears naturally in the title text — no special handling needed.
fn render_headline(props: &Value, children: &[Value], ctx: &RenderContext) -> String {
    let level = props
        .get("level")
        .and_then(|v| v.as_u64())
        .unwrap_or(1)
        .clamp(1, 6);

    // Title is a mixed array of strings and nested elements.
    // For TODO/DONE headlines, the keyword appears as plain text in this array.
    let title_html = props
        .get("title")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|item| render_node(item, ctx))
                .collect::<String>()
        })
        .unwrap_or_default();

    let heading_html = format!("<h{level}>{title_html}</h{level}>\n");
    let children_html = render_children(children, ctx);
    format!("{heading_html}{children_html}")
}

/// Render a link element.
/// Dispatches on link type: id (internal), https/http (external), file/fuzzy (plain text).
fn render_link(props: &Value, children: &[Value], ctx: &RenderContext) -> String {
    let link_type = props.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let raw_link = props.get("raw-link").and_then(|v| v.as_str()).unwrap_or("");
    let path = props.get("path").and_then(|v| v.as_str()).unwrap_or("");

    // Description: use children if present, otherwise raw-link as fallback
    let description = if children.is_empty() {
        html_escape(raw_link)
    } else {
        render_children(children, ctx)
    };

    match link_type {
        "id" => {
            // Resolve UUID to slug if available, fall back to raw UUID path
            let href = ctx
                .slug_map
                .get(path)
                .map(|s| format!("/{s}"))
                .unwrap_or_else(|| format!("/{path}"));
            format!("<a href=\"{href}\">{description}</a>")
        }
        "https" | "http" | "mailto" => {
            // Use raw-link (not path) — path strips the scheme prefix
            format!("<a href=\"{raw_link}\">{description}</a>")
        }
        "file" => {
            if let Some(web_url) = ctx.media_map.get(path) {
                let alt = if children.is_empty() {
                    html_escape(path.split('/').next_back().unwrap_or(path))
                } else {
                    render_children(children, ctx)
                };
                format!("<img src=\"{web_url}\" alt=\"{alt}\">")
            } else if path.to_lowercase().ends_with(".pdf") {
                let pdf_slug = std::path::Path::new(path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(path);
                format!("<div data-pdf-gallery-slug=\"{pdf_slug}\"></div>")
            } else {
                format!("<span class=\"file-link\">{description}</span>")
            }
        }
        _ => {
            // fuzzy, jira:, custom protocols — plain text
            description
        }
    }
}

/// Extract LAST_MODIFIED from the org-data AST node.
/// Primary: reads ast[1]["LAST_MODIFIED"] directly from org-data props.
/// Two org timestamp formats: "<2025-12-02 01:25>" and "<2026-01-16>".
/// Returns ISO 8601 UTC string, or epoch on failure.
pub fn extract_last_modified(ast: &Value) -> String {
    // Method 1 (preferred): org-data props directly
    if let Some(lm) = ast
        .get(1)
        .and_then(|props| props.get("LAST_MODIFIED"))
        .and_then(|v| v.as_str())
    {
        if let Some(parsed) = parse_org_timestamp(lm) {
            return parsed;
        }
    }

    // Method 2 (fallback): walk section > property-drawer > node-property
    if let Some(children) = ast.as_array() {
        for child in children.iter().skip(2) {
            if let Some(found) = walk_for_last_modified(child) {
                return found;
            }
        }
    }

    // Final fallback: epoch (per ADR-043)
    "1970-01-01T00:00:00Z".to_string()
}

/// Walk AST node looking for LAST_MODIFIED in property-drawers.
fn walk_for_last_modified(node: &Value) -> Option<String> {
    if let Value::Array(arr) = node {
        if arr.len() >= 2 {
            let typ = arr[0].as_str().unwrap_or("");
            if typ == "node-property" {
                let key = arr[1].get("key").and_then(|v| v.as_str()).unwrap_or("");
                if key == "LAST_MODIFIED" {
                    let value = arr[1].get("value").and_then(|v| v.as_str())?;
                    return parse_org_timestamp(value);
                }
            }
            // Recurse into children
            for child in arr.iter().skip(2) {
                if let Some(found) = walk_for_last_modified(child) {
                    return Some(found);
                }
            }
        }
    }
    None
}

/// Parse an org-mode timestamp string to ISO 8601 UTC.
/// Handles: "<2025-12-02 01:25>" and "<2026-01-16>".
fn parse_org_timestamp(s: &str) -> Option<String> {
    use chrono::{NaiveDate, NaiveDateTime};

    // Strip org angle brackets and optional day-of-week abbreviation
    let inner = s.trim_start_matches('<').trim_end_matches('>').trim();

    // Remove day-of-week abbreviation if present (e.g., "Mon", "Tue")
    // Format: "2026-01-16 Mon" or "2025-12-02 Mon 01:25"
    let parts: Vec<&str> = inner.splitn(2, ' ').collect();
    let date_part = parts[0];

    if parts.len() == 1 {
        // Date-only: "2026-01-16"
        NaiveDate::parse_from_str(date_part, "%Y-%m-%d")
            .ok()
            .map(|d| format!("{}", d.format("%Y-%m-%dT00:00:00Z")))
    } else {
        // May have day abbreviation and/or time
        // Try to find a time component: "HH:MM" pattern
        let rest = parts[1].trim();
        // rest could be: "Mon 01:25" or "01:25" or "Mon"
        let time_part = rest.split_whitespace().find(|p| p.contains(':'));

        if let Some(time) = time_part {
            let datetime_str = format!("{} {}", date_part, time);
            NaiveDateTime::parse_from_str(&datetime_str, "%Y-%m-%d %H:%M")
                .ok()
                .map(|dt| format!("{}", dt.format("%Y-%m-%dT%H:%M:%SZ")))
        } else {
            // Date with day abbreviation only
            NaiveDate::parse_from_str(date_part, "%Y-%m-%d")
                .ok()
                .map(|d| format!("{}", d.format("%Y-%m-%dT00:00:00Z")))
        }
    }
}

/// Map org-mode language identifiers to syntect syntax names.
/// Returns None if the language should fall back to plain rendering.
fn normalize_language(lang: &str) -> Option<&'static str> {
    match lang.to_lowercase().as_str() {
        "emacs-lisp" | "elisp" => Some("Lisp"),
        "sh" | "shell" | "bash" | "zsh" | "fish" => Some("Bash"),
        "js" | "javascript" => Some("JavaScript"),
        "ts" | "typescript" => Some("TypeScript"),
        "py" | "python" => Some("Python"),
        "rs" | "rust" => Some("Rust"),
        "rb" | "ruby" => Some("Ruby"),
        "hs" | "haskell" => Some("Haskell"),
        "css" => Some("CSS"),
        "html" | "htm" => Some("HTML"),
        "xml" => Some("XML"),
        "json" => Some("JSON"),
        "yaml" | "yml" => Some("YAML"),
        "toml" => Some("TOML"),
        "sql" => Some("SQL"),
        "c" => Some("C"),
        "cpp" | "c++" => Some("C++"),
        "go" => Some("Go"),
        "java" => Some("Java"),
        "kotlin" | "kt" => Some("Kotlin"),
        "scala" => Some("Scala"),
        "swift" => Some("Swift"),
        "r" => Some("R"),
        "lua" => Some("Lua"),
        "perl" | "pl" => Some("Perl"),
        "tex" | "latex" => Some("LaTeX"),
        "makefile" | "make" => Some("Makefile"),
        "diff" | "patch" => Some("Diff"),
        "ini" | "cfg" | "conf" => Some("INI"),
        "markdown" | "md" => Some("Markdown"),
        "nix" => Some("Nix"),
        // text, org, and other unknown languages fall back to plain
        _ => None,
    }
}

/// Render a src-block with syntax highlighting via syntect.
/// Falls back to plain HTML-escaped rendering if the language is unknown.
fn highlight_src_block(lang: &str, code: &str, ss: &SyntaxSet) -> String {
    // Try to find syntax; fall back to plain if unknown or empty
    let syntax = normalize_language(lang)
        .and_then(|name| ss.find_syntax_by_name(name))
        .or_else(|| {
            if lang.is_empty() {
                None
            } else {
                ss.find_syntax_by_token(lang)
            }
        });

    let Some(syntax) = syntax else {
        let escaped = html_escape(code);
        if lang.is_empty() {
            return format!("<pre><code>{escaped}</code></pre>\n");
        } else {
            return format!("<pre><code class=\"language-{lang}\">{escaped}</code></pre>\n");
        }
    };

    let mut generator = ClassedHTMLGenerator::new_with_class_style(syntax, ss, ClassStyle::Spaced);

    for line in LinesWithEndings::from(code) {
        let _ = generator.parse_html_for_line_which_includes_newline(line);
    }

    let highlighted = generator.finalize();
    format!("<pre><code class=\"language-{lang}\">{highlighted}</code></pre>\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use syntect::parsing::SyntaxSet;

    fn empty_ctx<'a>(
        slug_map: &'a SlugMap,
        media_map: &'a HashMap<String, String>,
        syntax_set: &'a SyntaxSet,
    ) -> RenderContext<'a> {
        RenderContext {
            slug_map,
            media_map,
            syntax_set,
        }
    }

    // REND-01: Render paragraph with plain string
    #[test]
    fn test_paragraph_plain_string() {
        let node = json!(["paragraph", null, "Hello world"]);
        let slugs = SlugMap::new();
        let media = HashMap::new();
        let ss = SyntaxSet::load_defaults_newlines();
        let ctx = empty_ctx(&slugs, &media, &ss);
        let result = render_node(&node, &ctx);
        assert!(result.contains("<p>"), "Expected <p> tag");
        assert!(result.contains("Hello world"), "Expected content");
    }

    // REND-02: Render headline level clamping
    #[test]
    fn test_headline_level_clamping() {
        let node = json!(["headline", {"level": 8, "title": ["Deep heading"]}, []]);
        let slugs = SlugMap::new();
        let media = HashMap::new();
        let ss = SyntaxSet::load_defaults_newlines();
        let ctx = empty_ctx(&slugs, &media, &ss);
        let result = render_node(&node, &ctx);
        assert!(result.contains("<h6>"), "Expected level clamped to h6");
        assert!(!result.contains("<h8>"), "Must not emit h8");
    }

    // REND-03: Render src-block with language
    #[test]
    fn test_src_block_with_language() {
        let node = json!(["src-block", {"language": "rust", "value": "fn main() {}"}, ]);
        let slugs = SlugMap::new();
        let media = HashMap::new();
        let ss = SyntaxSet::load_defaults_newlines();
        let ctx = empty_ctx(&slugs, &media, &ss);
        let result = render_node(&node, &ctx);
        assert!(result.contains("language-rust"), "Expected language class");
        assert!(result.contains("<pre>"), "Expected pre tag");
        assert!(result.contains("main"), "Expected code content");
    }

    // REND-04: Render internal id link resolves via slug map
    #[test]
    fn test_id_link_resolves_slug() {
        let mut slugs = SlugMap::new();
        slugs.insert(
            "ABCD1234-0000-0000-0000-000000000000".to_string(),
            "my-node".to_string(),
        );
        let node = json!(["link", {"type": "id", "raw-link": "id:ABCD1234-0000-0000-0000-000000000000", "path": "ABCD1234-0000-0000-0000-000000000000"}, "My Node"]);
        let media = HashMap::new();
        let ss = SyntaxSet::load_defaults_newlines();
        let ctx = empty_ctx(&slugs, &media, &ss);
        let result = render_node(&node, &ctx);
        assert!(
            result.contains("href=\"/my-node\""),
            "Expected resolved slug URL"
        );
    }

    // REND-05: Render external https link
    #[test]
    fn test_external_https_link() {
        let node = json!(["link", {"type": "https", "raw-link": "https://example.com", "path": "//example.com"}, "Example"]);
        let slugs = SlugMap::new();
        let media = HashMap::new();
        let ss = SyntaxSet::load_defaults_newlines();
        let ctx = empty_ctx(&slugs, &media, &ss);
        let result = render_node(&node, &ctx);
        assert!(
            result.contains("href=\"https://example.com\""),
            "Expected full https URL"
        );
    }

    // REND-05b: Render mailto link
    #[test]
    fn test_mailto_link() {
        let node = json!(["link", {"type": "mailto", "raw-link": "mailto:hello@example.com", "path": "hello@example.com"}, "Email me"]);
        let slugs = SlugMap::new();
        let media = HashMap::new();
        let ss = SyntaxSet::load_defaults_newlines();
        let ctx = empty_ctx(&slugs, &media, &ss);
        let result = render_node(&node, &ctx);
        assert!(
            result.contains("href=\"mailto:hello@example.com\""),
            "Expected mailto href"
        );
        assert!(result.contains("Email me"), "Expected link description");
    }

    // REND-06: Render file link as plain text span
    #[test]
    fn test_file_link_plain_text() {
        let node = json!(["link", {"type": "file", "raw-link": "file:/home/user/doc.org", "path": "/home/user/doc.org"}, "doc.org"]);
        let slugs = SlugMap::new();
        let media = HashMap::new();
        let ss = SyntaxSet::load_defaults_newlines();
        let ctx = empty_ctx(&slugs, &media, &ss);
        let result = render_node(&node, &ctx);
        assert!(
            !result.contains("<a href"),
            "file links must not produce anchor tags"
        );
        assert!(result.contains("file-link"), "Expected file-link span");
    }

    // REND-07: LAST_MODIFIED extraction when present in org-data props
    #[test]
    fn test_last_modified_from_org_data() {
        let ast = json!(["org-data", {"LAST_MODIFIED": "<2025-12-02 01:25>"}, []]);
        let result = extract_last_modified(&ast);
        assert_eq!(result, "2025-12-02T01:25:00Z");
    }

    // REND-08: LAST_MODIFIED absent returns epoch
    #[test]
    fn test_last_modified_absent_returns_epoch() {
        let ast = json!(["org-data", null, []]);
        let result = extract_last_modified(&ast);
        assert_eq!(result, "1970-01-01T00:00:00Z");
    }

    // REND-09: post-blank on inline elements emits trailing space
    #[test]
    fn test_post_blank_inline_spacing() {
        // Simulates: *markup* and → bold has post-blank:1, text " and" follows
        let node = json!(["paragraph", null,
            ["bold", {"post-blank": 1}, "markup"],
            "and"
        ]);
        let slugs = SlugMap::new();
        let media = HashMap::new();
        let ss = SyntaxSet::load_defaults_newlines();
        let ctx = empty_ctx(&slugs, &media, &ss);
        let result = render_node(&node, &ctx);
        assert!(
            result.contains("<strong>markup</strong> and"),
            "Expected space between </strong> and following text, got: {result}"
        );
    }

    // REND-10: Null properties on section must not panic
    #[test]
    fn test_null_props_section_no_panic() {
        let node = json!(["section", null, ["paragraph", null, "text"]]);
        let slugs = SlugMap::new();
        let media = HashMap::new();
        let ss = SyntaxSet::load_defaults_newlines();
        let ctx = empty_ctx(&slugs, &media, &ss);
        let result = render_node(&node, &ctx);
        assert!(result.contains("text"), "Expected children rendered");
    }

    // REND-11: Image file link renders as <img> when in media_map
    #[test]
    fn test_image_file_link_renders_img() {
        let mut media = HashMap::new();
        media.insert(
            "./images/puppy.png".to_string(),
            "/media/NODEID/puppy.png".to_string(),
        );
        let slugs = SlugMap::new();
        let ss = SyntaxSet::load_defaults_newlines();
        let ctx = RenderContext {
            slug_map: &slugs,
            media_map: &media,
            syntax_set: &ss,
        };
        let node = json!(["link", {"type": "file", "raw-link": "./images/puppy.png", "path": "./images/puppy.png"}]);
        let result = render_node(&node, &ctx);
        assert!(result.contains("<img"), "Expected <img> tag for image link");
        assert!(
            result.contains("src=\"/media/NODEID/puppy.png\""),
            "Expected correct src"
        );
        assert!(
            !result.contains("file-link"),
            "Must not emit file-link span for image"
        );
    }
}
