use std::env;
use std::io::{self, Read};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();

    let payload = if !args.is_empty() {
        args.join(" ")
    } else {
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;
        buffer.trim().to_string()
    };

    if payload.is_empty() {
        eprintln!("No text provided. Pass content as an argument or via stdin.");
        std::process::exit(1);
    }

    let base_url = env::var("COPYPASTE_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:8000".to_string());
    let base_url = base_url.trim_end_matches('/');

    let client = reqwest::blocking::Client::builder()
        .build()?;

    let endpoint = base_url.to_string();
    let response = client
        .post(&endpoint)
        .header("Content-Type", "text/plain")
        .body(payload)
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

    let full_url = if path.starts_with("http://") || path.starts_with("https://") {
        path.clone()
    } else {
        format!("{}{}", base_url, path)
    };

    println!("Paste link: {}", full_url);

    Ok(())
}
