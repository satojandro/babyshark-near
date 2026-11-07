//! OutLayer entry point: JSON request on stdin, JSON report on stdout.
//!
//! Runs as a WASI Preview 2 component inside Intel TDX. Optional secret
//! `GITHUB_TOKEN` (injected by OutLayer as an env var) raises GitHub's rate
//! limit; it never leaves the enclave.

use std::io::{self, Read, Write};

fn main() {
    let mut raw = String::new();
    let result = io::stdin()
        .read_to_string(&mut raw)
        .map_err(|e| format!("cannot read input: {e}"))
        .and_then(|_| run(&raw));

    let out = match result {
        Ok(report) => serde_json::json!({ "ok": true, "report": report }),
        Err(error) => serde_json::json!({ "ok": false, "error": error }),
    };
    print!("{out}");
    let _ = io::stdout().flush();
}

fn run(raw: &str) -> Result<babyshark_verifier::VerifyReport, String> {
    let req = babyshark_verifier::parse_request(raw).map_err(|e| e.to_string())?;
    let body = http_get(&babyshark_verifier::commits_url(&req))?;
    babyshark_verifier::summarize(&req, &body)
}

#[cfg(target_os = "wasi")]
fn http_get(url: &str) -> Result<String, String> {
    use std::time::Duration;
    let mut rb = wasi_http_client::Client::new()
        .get(url)
        .header("User-Agent", "babyshark-verifier")
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .connect_timeout(Duration::from_secs(10));
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        if !token.is_empty() {
            rb = rb.header("Authorization", format!("Bearer {token}"));
        }
    }
    let resp = rb.send().map_err(|e| format!("GitHub request failed: {e}"))?;
    let status = resp.status();
    let body = resp.body().map_err(|e| format!("GitHub body unreadable: {e}"))?;
    let text = String::from_utf8_lossy(&body).into_owned();
    if status != 200 {
        return Err(format!("GitHub answered {status}: {}", text.chars().take(200).collect::<String>()));
    }
    Ok(text)
}

#[cfg(not(target_os = "wasi"))]
fn http_get(_url: &str) -> Result<String, String> {
    Err("HTTP is only available inside the WASI build".into())
}
