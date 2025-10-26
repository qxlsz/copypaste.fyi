use std::io::{self, Read};

use clap::Parser;

/// Submit text to a copypaste.fyi instance and print the resulting URL.
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    /// Text to paste. When omitted, stdin is read instead.
    #[arg(conflicts_with = "stdin", required_unless_present = "stdin")]
    text: Option<String>,

    /// Read input from stdin.
    #[arg(long)]
    stdin: bool,

    /// Base URL of the copypaste server (e.g. http://127.0.0.1:8000).
    #[arg(long, default_value = "http://127.0.0.1:8000")]
    host: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let content = if cli.stdin {
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;
        buffer.trim().to_owned()
    } else {
        cli.text.unwrap()
    };

    if content.is_empty() {
        eprintln!("No input provided.");
        std::process::exit(1);
    }

    let base_url = cli.host.trim_end_matches('/');
    let client = reqwest::blocking::Client::builder().build()?;

    let response = client
        .post(base_url)
        .header("Content-Type", "text/plain")
        .body(content)
        .send()?;

    if !response.status().is_success() {
        eprintln!("Request failed with status: {}", response.status());
        std::process::exit(1);
    }

    let path = response.text()?.trim().to_string();
    if path.is_empty() {
        eprintln!("Server returned an empty response.");
        std::process::exit(1);
    }

    let url = format!("{}{}", base_url, path);
    println!("Paste link: {}", url);

    Ok(())
}
