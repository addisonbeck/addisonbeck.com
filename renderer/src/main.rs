use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "renderer", about = "Render org-roam export to HTML")]
struct Args {
    /// Path to the org-roam export directory
    #[arg(long, default_value = "~/.cache/org-roam-export")]
    input: String,

    /// Path to the rendered output directory
    #[arg(long, default_value = "rendered")]
    output: String,

    /// Render a single node UUID to stdout (debug mode)
    #[arg(long)]
    node: Option<String>,
}

fn main() {
    let args = Args::parse();

    if let Some(uuid) = &args.node {
        eprintln!("[renderer] single-node mode: {}", uuid);
        println!("TODO: render {}", uuid);
        return;
    }

    eprintln!("[renderer] input: {}", args.input);
    eprintln!("[renderer] output: {}", args.output);
    println!("TODO: full render pipeline");
}
