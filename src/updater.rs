/// Metadata for a single GitHub release asset, normalised across the
/// stable / pre-release / asset-only code paths.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReleaseInfo {
    pub tag: String,
    pub version: String,
    pub notes: String,
    pub asset_name: String,
    pub download_url: String,
    pub sha256: Option<String>,
}

/// Upstream app repository (used by the in-app updater).
pub const APP_REPO: &str = "lazy1104/Remotrix";

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

/// Fetch the latest stable (or beta) release of `repo`, picking the asset
/// whose name matches `"{asset_prefix}-{version}-{slug}"`. When
/// `fetch_checksum` is `true`, the matching `checksums.sha256` entry is
/// parsed from the same release.
///
/// # Errors
/// Returns an error string describing the network or GitHub API failure,
/// or "no matching asset" when no release contains the expected asset.
pub async fn fetch_latest_release(
    repo: &str,
    asset_prefix: &str,
    slug: &str,
    fetch_checksum: bool,
    proxy: Option<String>,
    beta: bool,
) -> Result<ReleaseInfo, String> {
    let client = http_client(proxy.as_deref())?;
    let not_found = format!("no matching asset '{asset_prefix}-*-{slug}' in latest release");

    if !beta {
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

        let prefix = asset_prefix.to_string();
        let slug = slug.to_string();
        release_from_json(
            &client,
            &body,
            move |name, version| name == format!("{prefix}-{version}-{slug}"),
            fetch_checksum,
            true,
        )
        .await
        .ok_or(not_found)
    } else {
        let prefix = asset_prefix.to_string();
        let slug = slug.to_string();
        fetch_beta_release(
            &client,
            repo,
            move |name, version| name == format!("{prefix}-{version}-{slug}"),
            fetch_checksum,
        )
        .await
        .ok_or(not_found)
    }
}

/// Fetch the latest release whose asset matches `asset_match` (suffix/kind based),
/// for installer-package releases that no longer ship a raw binary.
pub async fn fetch_latest_asset(
    repo: &str,
    asset_match: impl Fn(&str) -> bool,
    fetch_checksum: bool,
    proxy: Option<String>,
    beta: bool,
) -> Result<ReleaseInfo, String> {
    let client = http_client(proxy.as_deref())?;

    let info = if beta {
        fetch_beta_release(
            &client,
            repo,
            move |name, _| asset_match(name),
            fetch_checksum,
        )
        .await
    } else {
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

        release_from_json(
            &client,
            &body,
            move |name, _| asset_match(name),
            fetch_checksum,
            true,
        )
        .await
    };

    info.ok_or_else(|| "no matching installer asset in latest release".to_string())
}

/// Query the releases list endpoint (newest-first, includes pre-releases) and return
/// the first release whose asset matches. Used when beta channel is enabled.
async fn fetch_beta_release(
    client: &reqwest::Client,
    repo: &str,
    asset_match: impl Fn(&str, &str) -> bool,
    fetch_checksum: bool,
) -> Option<ReleaseInfo> {
    let api_url = format!("https://api.github.com/repos/{repo}/releases?per_page=5&page=1");
    let resp = client.get(&api_url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body: serde_json::Value = resp.json().await.ok()?;
    let releases = body.as_array()?;
    for rel in releases {
        if let Some(info) = release_from_json(client, rel, &asset_match, fetch_checksum, true).await
        {
            return Some(info);
        }
    }
    None
}

/// Page through GitHub releases (newest-first) and return every release
/// newer than `current` whose asset matches `asset_match`. Used to build
/// the changelog shown in the about dialog.
pub async fn fetch_changelog(
    repo: &str,
    current: String,
    asset_match: impl Fn(&str, &str) -> bool,
    proxy: Option<String>,
    beta: bool,
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
            let Some(info) = release_from_json(&client, rel, &asset_match, false, beta).await
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

/// Look up the sha256 entry for a specific (version, asset) pair inside a
/// GitHub release. Returns `None` if the asset's release does not ship
/// a `checksums.sha256`.
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
    asset_match: impl Fn(&str, &str) -> bool,
    fetch_checksum: bool,
    include_prerelease: bool,
) -> Option<ReleaseInfo> {
    let prerelease = body["prerelease"].as_bool().unwrap_or(false);
    if prerelease && !include_prerelease {
        return None;
    }
    let tag = body["tag_name"].as_str()?.to_string();
    let version = tag.strip_prefix('v').unwrap_or(&tag).to_string();
    let notes = body["body"].as_str().unwrap_or("").to_string();

    let assets = body["assets"].as_array()?;
    let asset = assets.iter().find(|a| {
        a["name"]
            .as_str()
            .map(|n| asset_match(n, &version))
            .unwrap_or(false)
    })?;

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

/// Slug used in release asset names for the current OS/architecture.
///
/// The slug matches the suffixes appended to GitHub asset names (e.g.
/// `aria2-next-1.2.3-linux-x86_64`).
///
/// # Panics
/// Panics when the running target is not one of the supported
/// `linux/{x86_64,aarch64}`, `macos/{x86_64,aarch64}` or
/// `windows/{x86_64,aarch64}` combinations.
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

/// Human-friendly `OS arch` string for the current target (e.g.
/// `"Linux x86_64"`, `"macOS arm64"`). Used by the about dialog and the
/// updater UI to make the platform obvious to non-technical users.
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

/// Parse a dotted version string into a numeric tuple. Non-numeric
/// segments (e.g. a `1.2.3-beta` suffix) are dropped, so the result may
/// be shorter than the input.
pub fn version_tuple(v: &str) -> Vec<u64> {
    v.split('.').filter_map(|p| p.parse::<u64>().ok()).collect()
}

/// Returns `true` when `a` is strictly newer than `b`.
///
/// The comparison is segment-by-segment with zero-padding: `1.2.0.1` is
/// newer than `1.2`, `1.10` is newer than `1.9`. Non-numeric segments
/// are ignored by [`version_tuple`], so `1.2.3-beta` and `1.2.3` are
/// considered equal.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_tuple_single_segment() {
        assert_eq!(version_tuple("5"), vec![5]);
    }

    #[test]
    fn version_tuple_multiple_segments() {
        assert_eq!(version_tuple("1.2.3"), vec![1, 2, 3]);
        assert_eq!(version_tuple("1.10.100"), vec![1, 10, 100]);
    }

    #[test]
    fn version_tuple_drops_non_numeric() {
        // "1.2.3-beta" → non-numeric "3-beta" segment can't parse, dropped.
        // Actually "-beta" attaches to "3", so "3-beta" fails to parse as u64.
        assert_eq!(version_tuple("1.2.3-beta"), vec![1, 2]);
        assert_eq!(version_tuple("1.2.3.4"), vec![1, 2, 3, 4]);
    }

    #[test]
    fn version_gt_basic() {
        assert!(version_gt("1.2.4", "1.2.3"));
        assert!(version_gt("2.0.0", "1.99.99"));
        assert!(!version_gt("1.2.3", "1.2.4"));
        assert!(!version_gt("1.2.3", "1.2.3"));
    }

    #[test]
    fn version_gt_unequal_lengths() {
        // Longer wins when prefix matches: 1.2.0.1 > 1.2
        assert!(version_gt("1.2.0.1", "1.2"));
        assert!(version_gt("1.10", "1.9"));
        assert!(!version_gt("1.2", "1.2.0"));
    }

    #[test]
    fn version_gt_ignores_non_numeric_suffix() {
        // 1.2.3-beta → tuple [1, 2]; equal to 1.2 → not greater.
        assert!(!version_gt("1.2.3-beta", "1.2"));
        assert!(!version_gt("1.2", "1.2.3-beta"));
    }

    #[test]
    fn platform_display_format() {
        let s = platform_display();
        // Always contains a space-separated OS and arch.
        assert!(s.contains(' '), "unexpected format: {s}");
    }
}
