// API proxy: tries local API first (localhost:8020), falls back to cloud.
// In standalone mode the local sidecar handles everything.
// Cloud fallback only used if local sidecar is unreachable.
const LOCAL_API_BASE: &str = "http://127.0.0.1:8020";
const REMOTE_API_BASE: &str = "https://auraalpha.cc";

/// IPC command: generic API proxy — tries local first, falls back to cloud.
#[tauri::command]
pub async fn api_proxy(
    method: String,
    path: String,
    body: Option<String>,
    auth_token: Option<String>,
) -> Result<String, String> {
    let rel_path = if path.starts_with("http") {
        // Absolute URL — use as-is
        return do_request(&method, &path, body.as_deref(), auth_token.as_deref(), 10).await;
    } else {
        path
    };

    let local_url = format!(
        "{}{}{}",
        LOCAL_API_BASE,
        if rel_path.starts_with('/') { "" } else { "/" },
        rel_path
    );

    // Try local API first (fast timeout)
    match do_request(&method, &local_url, body.as_deref(), auth_token.as_deref(), 5).await {
        Ok(result) => Ok(result),
        Err(_local_err) => {
            // Fall back to cloud (short timeout — proxied networks hang forever)
            let remote_url = format!(
                "{}{}{}",
                REMOTE_API_BASE,
                if rel_path.starts_with('/') { "" } else { "/" },
                rel_path
            );
            do_request(&method, &remote_url, body.as_deref(), auth_token.as_deref(), 3).await
        }
    }
}

async fn do_request(
    method: &str,
    url: &str,
    body: Option<&str>,
    auth_token: Option<&str>,
    timeout_secs: u64,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let mut req = match method.to_uppercase().as_str() {
        "POST" => client.post(url),
        "PUT" => client.put(url),
        "DELETE" => client.delete(url),
        "PATCH" => client.patch(url),
        _ => client.get(url),
    };

    req = req.timeout(std::time::Duration::from_secs(timeout_secs));
    req = req.header("Content-Type", "application/json");
    req = req.header("User-Agent", "AuraAlpha-Desktop/5.0.5");
    req = req.header("Accept", "application/json");

    if let Some(token) = auth_token {
        req = req.header("Authorization", format!("Bearer {}", token));
    }

    if let Some(b) = body {
        req = req.body(b.to_owned());
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
