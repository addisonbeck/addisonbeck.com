use clap::Parser;

mod types;

#[derive(Parser, Debug)]
#[command(name = "renderer", about = "Render org-roam export to HTML")]
struct Args {
    #[arg(long, default_value = "~/.cache/org-roam-export")]
    input: String,

    #[arg(long, default_value = "rendered")]
    output: String,

    #[arg(long)]
    node: Option<String>,
}

fn main() {
    let args = Args::parse();

    if let Some(uuid) = &args.node {
        let input_dir = expand_tilde(&args.input);
        let shard = &uuid[..2].to_lowercase();
        let path = format!("{}/{}/{}.json", input_dir, shard, uuid);
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Cannot read {}: {}", path, e));
        let node: types::NodeFile = serde_json::from_str(&content)
            .unwrap_or_else(|e| panic!("Cannot parse {}: {}", path, e));
        eprintln!("[renderer] parsed node: {} ({})", node.title, node.id);
        println!("TODO: render {}", uuid);
        return;
    }

    eprintln!("[renderer] input: {}", args.input);
    eprintln!("[renderer] output: {}", args.output);
    println!("TODO: full render pipeline");
}

/// Expand ~ to the home directory
fn expand_tilde(path: &str) -> String {
    if path.starts_with('~') {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        path.replacen('~', &home, 1)
    } else {
        path.to_string()
    }
}
