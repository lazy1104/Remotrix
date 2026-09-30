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
        tracing::debug!(%api_url, "update fetch: releases/latest");
        let resp = client
            .get(&api_url)
            .send()
            .await
            .map_err(|e| format!("fetch release: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            tracing::warn!(status = %status, "update fetch: non-success HTTP status");
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
        tracing::debug!(%api_url, "update fetch: releases/latest");
        let resp = client
            .get(&api_url)
            .send()
            .await
            .map_err(|e| format!("fetch release: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            tracing::warn!(status = %status, "update fetch: non-success HTTP status");
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
    tracing::debug!(%api_url, "update fetch: releases list");
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
        tracing::debug!(
            tag = %body["tag_name"].as_str().unwrap_or(""),
            "update fetch: skipping prerelease"
        );
        return None;
    }
    let tag = body["tag_name"].as_str()?.to_string();
    let version = tag.strip_prefix('v').unwrap_or(&tag).to_string();
    let notes = body["body"].as_str().unwrap_or("").to_string();

    let inline_assets = body["assets"].as_array();
    let mut assets_slice: Vec<serde_json::Value> = inline_assets.cloned().unwrap_or_default();
    if matches!(inline_assets, Some(a) if a.is_empty()) {
        if let Some(url) = body["assets_url"].as_str() {
            match fetch_authoritative_assets(client, url).await {
                Ok(authoritative) => {
                    tracing::warn!(
                        tag = %tag,
                        version = %version,
                        asset_count = authoritative.len(),
                        "update fetch: inline assets empty; fell back to assets_url"
                    );
                    assets_slice = authoritative;
                }
                Err(e) => {
                    tracing::warn!(
                        tag = %tag,
                        version = %version,
                        error = %e,
                        "update fetch: assets_url fallback failed"
                    );
                }
            }
        }
    }

    let mut info =
        build_release_info(tag, version, notes, prerelease, &assets_slice, &asset_match)?;
    if fetch_checksum {
        info.sha256 = try_fetch_checksum(client, &info.asset_name, &assets_slice).await;
    }
    Some(info)
}

fn build_release_info(
    tag: String,
    version: String,
    notes: String,
    prerelease: bool,
    assets: &[serde_json::Value],
    asset_match: &dyn Fn(&str, &str) -> bool,
) -> Option<ReleaseInfo> {
    let asset = assets.iter().find(|a| {
        a["name"]
            .as_str()
            .map(|n| asset_match(n, &version))
            .unwrap_or(false)
    })?;
    let download_url = asset["browser_download_url"].as_str()?.to_string();
    let asset_name = asset["name"].as_str()?.to_string();
    tracing::debug!(
        tag = %tag,
        version = %version,
        asset_name = %asset_name,
        prerelease,
        "update fetch: release matched"
    );
    Some(ReleaseInfo {
        tag,
        version,
        notes,
        asset_name,
        download_url,
        sha256: None,
    })
}

async fn fetch_authoritative_assets(
    client: &reqwest::Client,
    url: &str,
) -> Result<Vec<serde_json::Value>, String> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("fetch assets_url: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        tracing::warn!(status = %status, "update fetch: assets_url non-success HTTP status");
        return Err(format!("assets_url HTTP {status}: {body}"));
    }
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("parse assets json: {e}"))?;
    body.as_array()
        .cloned()
        .ok_or_else(|| "expected assets array".to_string())
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

/// Split the dotted numeric core (`MAJOR.MINOR.PATCH[.extras]`) from a
/// trailing `-pre` suffix. The prerelease marker is the first `-`, so
/// `0.6.0-beta.1` yields `([0, 6, 0], Some("beta.1"))` and the trailing
/// `1` of the prerelease is never absorbed into the core tuple.
fn split_prerelease(v: &str) -> (Vec<u64>, Option<&str>) {
    let (core, pre) = match v.find('-') {
        Some(i) => (&v[..i], Some(&v[i + 1..])),
        None => (v, None),
    };
    let tuple = core
        .split('.')
        .filter_map(|p| p.parse::<u64>().ok())
        .collect();
    (tuple, pre)
}

/// Parse a dotted version string into a numeric tuple. The prerelease
/// suffix (everything after the first `-`) is stripped, so `1.2.3-beta`
/// and `1.2.3-beta.1` both yield `[1, 2, 3]`.
pub fn version_tuple(v: &str) -> Vec<u64> {
    split_prerelease(v).0
}

/// Order two prerelease identifiers of the same numeric core (SemVer-style).
/// Numeric identifiers compare numerically, identifiers with letters compare
/// lexically, and a longer identifier list wins only when the shared prefix
/// is equal. Returns `true` when `a` sorts after `b`.
fn prerelease_gt(a: &str, b: &str) -> bool {
    let pa: Vec<&str> = a.split('.').collect();
    let pb: Vec<&str> = b.split('.').collect();
    let len = pa.len().max(pb.len());
    for i in 0..len {
        let x = pa.get(i).copied().unwrap_or("");
        let y = pb.get(i).copied().unwrap_or("");
        if x.is_empty() && y.is_empty() {
            return false;
        }
        if y.is_empty() {
            return true;
        }
        if x.is_empty() {
            return false;
        }
        match (x.parse::<u64>(), y.parse::<u64>()) {
            (Ok(xn), Ok(yn)) if xn != yn => return xn > yn,
            (Ok(_), Ok(_)) => {}
            _ => match x.cmp(y) {
                std::cmp::Ordering::Equal => {}
                ord => return ord.is_gt(),
            },
        }
    }
    false
}

/// Returns `true` when `a` is strictly newer than `b`.
///
/// The numeric core is compared segment-by-segment with zero-padding:
/// `1.2.0.1` is newer than `1.2`, `1.10` is newer than `1.9`. A prerelease
/// (`-beta.1`) is the same target as its base release, so `0.6.0-beta.1`
/// is **not** newer than `0.6.0`, while `0.6.0` is newer than
/// `0.6.0-beta.1`. Prereleases of the same core are ordered by
/// [`prerelease_gt`] (e.g. `0.6.0-beta.2` newer than `0.6.0-beta.1`).
pub fn version_gt(a: &str, b: &str) -> bool {
    let (ta, pa) = split_prerelease(a);
    let (tb, pb) = split_prerelease(b);
    let len = ta.len().max(tb.len());
    for i in 0..len {
        let x = ta.get(i).copied().unwrap_or(0);
        let y = tb.get(i).copied().unwrap_or(0);
        if x != y {
            return x > y;
        }
    }
    match (pa, pb) {
        (None, None) => false,
        (Some(pa), Some(pb)) => prerelease_gt(pa, pb),
        (Some(_), None) => false,
        (None, Some(_)) => true,
    }
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
        // The prerelease marker (first '-') splits off the numeric core, so
        // "1.2.3-beta" and "1.2.3-beta.1" share the base tuple [1, 2, 3].
        assert_eq!(version_tuple("1.2.3-beta"), vec![1, 2, 3]);
        assert_eq!(version_tuple("1.2.3-beta.1"), vec![1, 2, 3]);
        assert_eq!(version_tuple("0.6.0-beta.1"), vec![0, 6, 0]);
        assert_eq!(version_tuple("1.2.3.4"), vec![1, 2, 3, 4]);
        assert_eq!(version_tuple("1.10.100"), vec![1, 10, 100]);
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
    fn version_gt_prerelease_not_newer_than_release() {
        // The reported bug: a prerelease of the same target must not be
        // offered as an update over the final release.
        assert!(!version_gt("0.6.0-beta.1", "0.6.0"));
        assert!(version_gt("0.6.0", "0.6.0-beta.1"));
        assert!(!version_gt("0.6.0-beta.1", "0.6.0-beta.1"));
        assert!(!version_gt("1.2.3-beta", "1.2.3"));
        // Beta of an older target is less mature: not newer than a lower
        // base release either.
        assert!(!version_gt("0.5.0-beta.9", "0.6.0"));
        assert!(version_gt("0.6.0-beta.1", "0.5.0"));
    }

    #[test]
    fn version_gt_prerelease_order_same_core() {
        // Within the same core, a later snapshot is newer.
        assert!(version_gt("0.6.0-beta.2", "0.6.0-beta.1"));
        assert!(!version_gt("0.6.0-beta.1", "0.6.0-beta.2"));
    }

    #[test]
    fn version_gt_total_order() {
        // 0.6.0-beta.1 < 0.6.0-beta.2 < 0.6.0 < 0.6.1-beta.1
        assert!(version_gt("0.6.0-beta.2", "0.6.0-beta.1"));
        assert!(version_gt("0.6.0", "0.6.0-beta.2"));
        assert!(!version_gt("0.6.0-beta.2", "0.6.0"));
        assert!(version_gt("0.6.1-beta.1", "0.6.0"));
        assert!(!version_gt("0.6.0-beta.1", "0.6.0-beta.1"));
    }

    #[test]
    fn platform_display_format() {
        let s = platform_display();
        // Always contains a space-separated OS and arch.
        assert!(s.contains(' '), "unexpected format: {s}");
    }

    fn deb_match(name: &str, _version: &str) -> bool {
        name.ends_with(".deb")
    }

    fn deb_asset(name: &str, url: &str) -> serde_json::Value {
        serde_json::json!({
            "name": name,
            "browser_download_url": url,
            "state": "uploaded",
        })
    }

    #[test]
    fn build_release_info_matches_inline_deb() {
        let assets = serde_json::json!([
            deb_asset("remotrix_0.5.1_amd64.deb", "https://x.example/deb"),
            deb_asset("checksums.sha256", "https://x.example/sha"),
        ]);
        let info = build_release_info(
            "v0.5.1".into(),
            "0.5.1".into(),
            String::new(),
            false,
            assets.as_array().expect("array"),
            &deb_match,
        )
        .expect("should match inline .deb");
        assert_eq!(info.version, "0.5.1");
        assert_eq!(info.tag, "v0.5.1");
        assert_eq!(info.asset_name, "remotrix_0.5.1_amd64.deb");
        assert_eq!(info.download_url, "https://x.example/deb");
        assert!(info.sha256.is_none());
    }

    #[test]
    fn build_release_info_handles_empty_assets() {
        let empty: Vec<serde_json::Value> = vec![];
        let info = build_release_info(
            "v0.5.1".into(),
            "0.5.1".into(),
            String::new(),
            false,
            &empty,
            &deb_match,
        );
        assert!(info.is_none(), "empty assets must not match");
    }

    #[test]
    fn build_release_info_returns_none_when_no_asset_matches() {
        let assets = serde_json::json!([
            deb_asset("only-exe.exe", "https://x.example/exe"),
            deb_asset("checksums.sha256", "https://x.example/sha"),
        ]);
        let info = build_release_info(
            "v0.5.1".into(),
            "0.5.1".into(),
            String::new(),
            false,
            assets.as_array().expect("array"),
            &deb_match,
        );
        assert!(
            info.is_none(),
            "no .deb in the assets list should not match a .deb matcher"
        );
    }

    #[test]
    fn build_release_info_matches_authoritative_after_empty_inline() {
        // Simulates the GitHub quirk where the embedded `assets` array is
        // empty but the assets themselves exist on the dedicated endpoint.
        let empty_inline: Vec<serde_json::Value> = vec![];
        let authoritative = serde_json::json!([deb_asset(
            "remotrix_0.5.1_amd64.deb",
            "https://x.example/deb"
        ),]);
        let info = build_release_info(
            "v0.5.1".into(),
            "0.5.1".into(),
            String::new(),
            false,
            authoritative.as_array().expect("array"),
            &deb_match,
        )
        .expect("authoritative .deb must match");
        assert_eq!(info.version, "0.5.1");
        assert_eq!(info.asset_name, "remotrix_0.5.1_amd64.deb");
        assert_eq!(info.download_url, "https://x.example/deb");
        // Inline being empty is the caller's signal to swap in this slice;
        // the matching function itself only sees the slice it was given.
        let _ = empty_inline;
    }

    #[test]
    fn build_release_info_prefers_first_matching_asset() {
        let assets = serde_json::json!([
            deb_asset("checksums.sha256", "https://x.example/sha"),
            deb_asset("remotrix_0.5.1_amd64.deb", "https://x.example/deb"),
            deb_asset("remotrix_0.5.1_x86_64.AppImage", "https://x.example/img"),
        ]);
        let info = build_release_info(
            "v0.5.1".into(),
            "0.5.1".into(),
            String::new(),
            false,
            assets.as_array().expect("array"),
            &deb_match,
        )
        .expect("should match the second .deb");
        assert_eq!(info.asset_name, "remotrix_0.5.1_amd64.deb");
    }
}
