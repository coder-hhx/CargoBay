// ── Auto-update ────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
struct UpdateInfo {
    available: bool,
    current_version: String,
    latest_version: String,
    release_notes: String,
    download_url: String,
}

#[tauri::command]
async fn check_update() -> Result<UpdateInfo, String> {
    let current_version = env!("CARGO_PKG_VERSION");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Use redirect-based approach first (no rate limit), fall back to API
    let tag = match client
        .head("https://github.com/coder-hhx/CrateBay/releases/latest")
        .header("User-Agent", format!("CrateBay/{}", current_version))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_redirection() => {
            // Location header: https://github.com/.../releases/tag/v1.2.3
            resp.headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .filter(|url| url.contains("/releases/tag/"))
                .and_then(|url| url.rsplit('/').next())
                .map(|t| t.trim_start_matches('v').to_string())
                .unwrap_or_default()
        }
        _ => String::new(),
    };

    // If redirect approach failed, try API as fallback
    let (tag, release_notes, html_url) = if tag.is_empty() {
        let resp = client
            .get("https://api.github.com/repos/coder-hhx/CrateBay/releases/latest")
            .header("User-Agent", format!("CrateBay/{}", current_version))
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await
            .map_err(|e| format!("Failed to fetch release info: {}", e))?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            // No releases published yet — treat as up-to-date
            return Ok(UpdateInfo {
                available: false,
                current_version: current_version.to_string(),
                latest_version: current_version.to_string(),
                release_notes: String::new(),
                download_url: String::new(),
            });
        }

        if !resp.status().is_success() {
            return Err(format!("GitHub API returned status {}", resp.status()));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse release info: {}", e))?;

        let api_tag = body["tag_name"]
            .as_str()
            .unwrap_or("")
            .trim_start_matches('v')
            .to_string();
        let notes = body["body"].as_str().unwrap_or("").to_string();
        let url = body["html_url"].as_str().unwrap_or("").to_string();
        (api_tag, notes, url)
    } else {
        let url = format!(
            "https://github.com/coder-hhx/CrateBay/releases/tag/v{}",
            tag
        );
        (tag, String::new(), url)
    };

    let available = !tag.is_empty() && tag != current_version;

    Ok(UpdateInfo {
        available,
        current_version: current_version.to_string(),
        latest_version: if tag.is_empty() {
            current_version.to_string()
        } else {
            tag
        },
        release_notes,
        download_url: html_url,
    })
}

#[tauri::command]
async fn open_release_page(url: String) -> Result<(), String> {
    open::that(&url).map_err(|e| format!("Failed to open URL: {}", e))
}

#[tauri::command]
async fn set_window_theme(window: tauri::WebviewWindow, theme: String) -> Result<(), String> {
    let t = match theme.as_str() {
        "light" => Some(tauri::Theme::Light),
        "dark" => Some(tauri::Theme::Dark),
        _ => None,
    };
    window.set_theme(t).map_err(|e| e.to_string())
}
