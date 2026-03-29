// Hit the origin server directly — bypasses Cloudflare bot protection.
// Cloudflare blocks non-browser HTTP clients (reqwest, curl) with Bot Fight Mode.
// The desktop app uses this direct connection; the web app goes through Cloudflare.
const REMOTE_API_BASE: &str = "https://auraalpha.cc";

/// IPC command: generic API proxy — bypasses browser CORS by using reqwest (Rust-side HTTP).
/// The frontend calls this instead of fetch() to avoid Cloudflare CORS issues.
#[tauri::command]
pub async fn api_proxy(
    method: String,
    path: String,
    body: Option<String>,
    auth_token: Option<String>,
) -> Result<String, String> {
    let url = if path.starts_with("http") {
        path
    } else {
        format!(
            "{}{}{}",
            REMOTE_API_BASE,
            if path.starts_with('/') { "" } else { "/" },
            path
        )
    };

    let client = reqwest::Client::new();
    let mut req = match method.to_uppercase().as_str() {
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "DELETE" => client.delete(&url),
        "PATCH" => client.patch(&url),
        _ => client.get(&url),
    };

    req = req.timeout(std::time::Duration::from_secs(30));
    req = req.header("Content-Type", "application/json");
    req = req.header("User-Agent", "AuraAlpha-Desktop/5.0.0");
    req = req.header("Accept", "application/json");

    if let Some(token) = auth_token {
        req = req.header("Authorization", format!("Bearer {}", token));
    }

    if let Some(b) = body {
        req = req.body(b);
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            if status >= 200 && status < 300 {
                Ok(text)
            } else {
                Err(format!("API {}: {}", status, text))
            }
        }
        Err(e) => Err(format!("Connection failed: {}", e)),
    }
}
