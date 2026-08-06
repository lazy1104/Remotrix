#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReleaseInfo {
    pub tag: String,
    pub version: String,
    pub notes: String,
    pub asset_name: String,
    pub download_url: String,
    pub sha256: Option<String>,
}

pub const APP_REPO: &str = "lazy1104/Remotrix";
pub const APP_ASSET_PREFIX: &str = "remotrix";

fn http_client(proxy: Option<&str>) -> Result<reqwest::Client, String> {
    let builder = crate::config::apply_proxy(
        reqwest::Client::builder()
            .user_agent("remotrix-updater")
            .timeout(std::time::Duration::from_secs(30)),
        proxy,
    )?;
    builder
        .build()
        .map_err(|e| format!("create reqwest client: {e}"))
}

pub async fn fetch_latest_release(
    repo: &str,
    asset_prefix: &str,
    slug: &str,
    fetch_checksum: bool,
    proxy: Option<String>,
) -> Result<ReleaseInfo, String> {
    let client = http_client(proxy.as_deref())?;
    let api_url = format!("https://api.github.com/repos/{repo}/releases/latest");
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

    release_from_json(&client, &body, asset_prefix, slug, fetch_checksum)
        .await
        .ok_or_else(|| format!("no matching asset '{asset_prefix}-*-{slug}' in latest release"))
}

pub async fn fetch_changelog(
    repo: &str,
    asset_prefix: &str,
    slug: &str,
    current: String,
    proxy: Option<String>,
) -> Result<Vec<ReleaseInfo>, String> {
    let client = http_client(proxy.as_deref())?;
    let mut out = Vec::new();
    let mut page = 1u32;
    // Fetch a few releases at a time (releases are newest-first) and stop as soon
    // as we reach a version that is not newer than `current`, so we only pull the
    // pages needed to cover the releases this component is actually behind on.
    loop {
        let api_url =
            format!("https://api.github.com/repos/{repo}/releases?per_page=5&page={page}");
        let resp = client
            .get(&api_url)
            .send()
            .await
            .map_err(|e| format!("fetch releases: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("GitHub API HTTP {status}: {body}"));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("parse releases json: {e}"))?;
        let releases = body
            .as_array()
            .ok_or_else(|| "expected releases array".to_string())?;
        if releases.is_empty() {
            break;
        }

        for rel in releases {
            let Some(info) = release_from_json(&client, rel, asset_prefix, slug, false).await
            else {
                continue;
            };
            if version_gt(&info.version, &current) {
                out.push(info);
            } else {
                // Releases are newest-first, so nothing later can be newer either.
                return Ok(out);
            }
        }
        page += 1;
    }
    Ok(out)
}

pub async fn fetch_asset_checksum(
    repo: &str,
    version: &str,
    asset_name: &str,
    proxy: Option<String>,
) -> Option<String> {
    let client = http_client(proxy.as_deref()).ok()?;
    let tag = version.strip_prefix('v').unwrap_or(version);
    let api_url = format!("https://api.github.com/repos/{repo}/releases/tags/v{tag}");
    let resp = client.get(&api_url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body: serde_json::Value = resp.json().await.ok()?;
    let assets = body["assets"].as_array()?;
    try_fetch_checksum(&client, asset_name, assets).await
}

async fn release_from_json(
    client: &reqwest::Client,
    body: &serde_json::Value,
    asset_prefix: &str,
    slug: &str,
    fetch_checksum: bool,
) -> Option<ReleaseInfo> {
    let tag = body["tag_name"].as_str()?.to_string();
    let version = tag.strip_prefix('v').unwrap_or(&tag).to_string();
    let notes = body["body"].as_str().unwrap_or("").to_string();

    let expected_name = format!("{asset_prefix}-{version}-{slug}");
    let assets = body["assets"].as_array()?;
    let asset = assets
        .iter()
        .find(|a| a["name"].as_str() == Some(&expected_name))?;

    let download_url = asset["browser_download_url"].as_str()?.to_string();
    let asset_name = asset["name"].as_str()?.to_string();
    let sha256 = if fetch_checksum {
        try_fetch_checksum(client, &asset_name, assets).await
    } else {
        None
    };

    Some(ReleaseInfo {
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

pub fn platform_display() -> String {
    let os = match std::env::consts::OS {
        "linux" => "Linux",
        "macos" => "macOS",
        "windows" => "Windows",
        other => other,
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" => "arm64",
        other => other,
    };
    format!("{os} {arch}")
}

pub fn version_tuple(v: &str) -> Vec<u64> {
    v.split('.').filter_map(|p| p.parse::<u64>().ok()).collect()
}

pub fn version_gt(a: &str, b: &str) -> bool {
    let ta = version_tuple(a);
    let tb = version_tuple(b);
    let len = ta.len().max(tb.len());
    for i in 0..len {
        let x = ta.get(i).copied().unwrap_or(0);
        let y = tb.get(i).copied().unwrap_or(0);
        if x != y {
            return x > y;
        }
    }
    false
}
