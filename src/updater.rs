#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReleaseInfo {
    pub tag: String,
    pub version: String,
    pub notes: String,
    pub asset_name: String,
    pub download_url: String,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone)]
pub enum CheckOutcome {
    UpToDate {
        version: String,
    },
    NewVersion {
        current: String,
        release: ReleaseInfo,
    },
    Error {
        message: String,
    },
}

pub async fn fetch_latest_release(repo: &str, slug: &str) -> Result<ReleaseInfo, String> {
    let api_url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let client = reqwest::Client::builder()
        .user_agent("remotrix-updater")
        .build()
        .map_err(|e| format!("create reqwest client: {e}"))?;

    let resp = client
        .get(&api_url)
        .send()
        .await
        .map_err(|e| format!("fetch release: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("GitHub API HTTP {status}: {body}"));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("parse release json: {e}"))?;

    let tag = body["tag_name"]
        .as_str()
        .ok_or_else(|| "missing tag_name".to_string())?
        .to_string();
    let version = tag.strip_prefix('v').unwrap_or(&tag).to_string();
    let notes = body["body"].as_str().unwrap_or("").to_string();

    let expected_name = format!("aria2-next-{version}-{slug}");
    let assets = body["assets"]
        .as_array()
        .ok_or_else(|| "missing assets".to_string())?;

    let asset = assets
        .iter()
        .find(|a| a["name"].as_str() == Some(&expected_name))
        .ok_or_else(|| {
            let names: Vec<&str> = assets.iter().filter_map(|a| a["name"].as_str()).collect();
            format!(
                "asset '{expected_name}' not found among: {}",
                names.join(", ")
            )
        })?;

    let download_url = asset["browser_download_url"]
        .as_str()
        .ok_or_else(|| "missing download_url".to_string())?
        .to_string();
    let asset_name = asset["name"]
        .as_str()
        .ok_or_else(|| "missing asset name".to_string())?
        .to_string();

    let sha256 = try_fetch_checksum(&client, &asset_name, assets).await;

    Ok(ReleaseInfo {
        tag,
        version,
        notes,
        asset_name,
        download_url,
        sha256,
    })
}

async fn try_fetch_checksum(
    client: &reqwest::Client,
    asset_name: &str,
    assets: &[serde_json::Value],
) -> Option<String> {
    let checksum_asset = assets.iter().find(|a| {
        a["name"]
            .as_str()
            .map(|n| n == "checksums.sha256" || n.ends_with(".sha256"))
            .unwrap_or(false)
    })?;

    let checksum_url = checksum_asset["browser_download_url"].as_str()?;
    let checksum_text = client
        .get(checksum_url)
        .send()
        .await
        .ok()?
        .text()
        .await
        .ok()?;

    for line in checksum_text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let filename = parts[1].trim_start_matches('*');
            if filename == asset_name {
                return Some(parts[0].to_string());
            }
        }
    }
    None
}

pub fn platform_slug() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "linux-x86_64",
        ("linux", "aarch64") => "linux-aarch64",
        ("macos", "aarch64") => "macos-arm64",
        ("macos", "x86_64") => "macos-x86_64",
        ("windows", "x86_64") => "windows-x86_64.exe",
        ("windows", "aarch64") => "windows-arm64.exe",
        _ => panic!(
            "unsupported platform: {}-{}",
            std::env::consts::OS,
            std::env::consts::ARCH
        ),
    }
}
