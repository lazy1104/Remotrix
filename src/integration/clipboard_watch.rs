//! Detects whether a clipboard payload (text or a pasted file path) should be
//! offered as a download. Used by the periodic clipboard watcher in `app.rs`
//! to decide whether a new payload is worth forwarding to the "Add download"
//! dialog. URLs are classified by scheme (http / ftp / magnet / ed2k /
//! thunder), and bare BitTorrent info-hashes are lifted into `magnet:` links.

use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::engine::is_metalink_file as is_metalink_path;

/// Maximum clipboard payload size, in bytes, that the watcher will inspect
/// for either inline text or pasted file content. 64 KiB matches the upper
/// bound we are willing to allocate on the watcher thread; anything larger
/// is ignored to avoid unbounded memory use when users copy huge blobs (e.g.
/// a binary file marked as text).
pub const MAX_CLIPBOARD_CONTENT: u64 = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ClipboardLinkTypes {
    /// Whether `http://` / `https://` URLs are extracted from clipboard text.
    #[serde(default = "default_true")]
    pub http: bool,
    /// Whether `ftp://` / `ftps://` URLs are extracted from clipboard text.
    #[serde(default = "default_true")]
    pub ftp: bool,
    /// Whether `magnet:?xt=urn:btih:...` links are extracted.
    #[serde(default = "default_true")]
    pub magnet: bool,
    /// Whether `ed2k://` links are extracted.
    #[serde(default = "default_true")]
    pub ed2k: bool,
    /// Whether `thunder://` links are extracted and base64-decoded into the
    /// underlying URL they wrap.
    #[serde(default = "default_true")]
    pub thunder: bool,
    /// Whether bare 40-char hex / 32-char base32 BitTorrent info-hashes are
    /// detected and lifted into `magnet:?xt=urn:btih:<hash>` links.
    #[serde(default = "default_true")]
    pub bt_infohash: bool,
    /// Whether `metalink://` / `meta4://` links are extracted (rare; most
    /// metalink clipboard payloads are HTTPS URLs ending in `.metalink` /
    /// `.meta4` and are already caught by the generic URL extractor).
    #[serde(default = "default_true")]
    pub metalink: bool,
}

fn default_true() -> bool {
    true
}

impl Default for ClipboardLinkTypes {
    fn default() -> Self {
        Self {
            http: true,
            ftp: true,
            magnet: true,
            ed2k: true,
            thunder: true,
            bt_infohash: true,
            metalink: true,
        }
    }
}

/// How aggressively the clipboard watcher should filter out plain webpage
/// URLs that aren't actually downloads.
///
/// `Smart` (default) keeps URLs whose file extension or query parameters
/// scream "download" and asynchronously probes the rest with a single HEAD
/// request to inspect `Content-Type` / `Content-Disposition`, mirroring how
/// a browser decides whether to download or render. `Static` skips probing
/// entirely (silent on the wire) but drops ambiguous extension-less URLs.
/// `Off` disables all filtering — every http/https URL surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum WebpageFilterMode {
    #[default]
    Smart,
    Off,
    Static,
}

impl WebpageFilterMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Smart => "smart",
            Self::Off => "off",
            Self::Static => "static",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "smart" => Some(Self::Smart),
            "off" => Some(Self::Off),
            "static" => Some(Self::Static),
            _ => None,
        }
    }
}

/// Outcome of classifying a URL's response or its static fingerprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentDecision {
    /// Definite download (Content-Disposition: attachment, octet-stream,
    /// known archive / media content type, …).
    Download,
    /// Renderable HTML page; do not offer as a download.
    Webpage,
    /// Neither clearly a download nor a webpage — keep the URL in the
    /// candidate list rather than silently dropping it.
    Ambiguous,
}

/// What the clipboard currently looks like to the watcher.
///
/// Returned from [`parse_clipboard`]. Single-line payloads that resolve to an
/// existing local `.torrent` file are surfaced as [`ClipboardPayload::Torrent`]
/// (so the caller can hand the file to aria2 directly); everything else is
/// classified into URLs.
#[derive(Debug, Clone, PartialEq)]
pub enum ClipboardPayload {
    /// One or more download links extracted from the clipboard text (or from
    /// the contents of a small pasted file). Order reflects first appearance
    /// in the source text after de-duplication.
    Urls(Vec<String>),
    /// A single local `.torrent` file path that the user appears to have
    /// pasted on the clipboard.
    Torrent(PathBuf),
    /// A single local `.metalink` / `.meta4` file path that the user appears
    /// to have pasted on the clipboard.
    Metalink(PathBuf),
}

/// Parse clipboard text into a [`ClipboardPayload`] if it contains anything
/// worth downloading.
///
/// Returns `None` if the payload exceeds [`MAX_CLIPBOARD_CONTENT`], if no
/// enabled link type matched, or if a single-line paste that looked like a
/// file path turned out to be unreadable / over the size cap / non-UTF-8.
/// On the single-line path, a file with a `.torrent` extension is returned
/// as [`ClipboardPayload::Torrent`] without inspecting its contents.
pub fn parse_clipboard(text: &str, prefs: ClipboardLinkTypes) -> Option<ClipboardPayload> {
    if text.len() as u64 > MAX_CLIPBOARD_CONTENT {
        return None;
    }
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    if lines.len() == 1 {
        if let Some((path, len)) = file_path_from_line(lines[0]) {
            if is_torrent_path(&path) {
                return Some(ClipboardPayload::Torrent(path));
            }
            if is_metalink_path(&path) {
                return Some(ClipboardPayload::Metalink(path));
            }
            return file_content_links(&path, len, prefs);
        }
    }
    let urls = extract_links(text, prefs);
    if urls.is_empty() {
        None
    } else {
        Some(ClipboardPayload::Urls(urls))
    }
}

/// Compute a stable hex-encoded SHA-256 fingerprint of a clipboard payload,
/// used to deduplicate consecutive updates from the OS clipboard watcher.
///
/// Tags the input with the payload variant (`urls|...` vs `torrent|...`) so
/// that an identical URL set and a torrent path sharing bytes cannot collide.
/// Returns an empty string for `None`, which the watcher treats as "no
/// payload" and is distinct from any real hash.
pub fn payload_hash(payload: &Option<ClipboardPayload>) -> String {
    match payload {
        Some(ClipboardPayload::Urls(urls)) => hex::encode(Sha256::digest(
            format!("urls|{}", urls.join("\n")).as_bytes(),
        )),
        Some(ClipboardPayload::Torrent(path)) => hex::encode(Sha256::digest(
            format!("torrent|{}", path.display()).as_bytes(),
        )),
        Some(ClipboardPayload::Metalink(path)) => hex::encode(Sha256::digest(
            format!("metalink|{}", path.display()).as_bytes(),
        )),
        None => String::new(),
    }
}

/// Compute a stable hex-encoded SHA-256 fingerprint of the raw clipboard
/// text, independent of how the text is later classified or filtered. Used
/// to dedupe consecutive clipboard updates so that a webpage URL we just
/// decided to drop doesn't trigger another probe every time the window
/// regains focus.
pub fn text_hash(text: &str) -> String {
    hex::encode(Sha256::digest(format!("text|{text}").as_bytes()))
}

struct LinkMatch {
    start: usize,
    end: usize,
    value: String,
}

fn extract_links(text: &str, prefs: ClipboardLinkTypes) -> Vec<String> {
    let mut matches: Vec<LinkMatch> = Vec::new();

    if prefs.http {
        scan_prefixes(
            text,
            &["http://", "https://"],
            |s| s.to_string(),
            &mut matches,
        );
    }
    if prefs.ftp {
        scan_prefixes(
            text,
            &["ftp://", "ftps://"],
            |s| s.to_string(),
            &mut matches,
        );
    }
    if prefs.magnet {
        scan_prefixes(text, &["magnet:?"], |s| s.to_string(), &mut matches);
    }
    if prefs.ed2k {
        scan_prefixes(text, &["ed2k://"], |s| s.to_string(), &mut matches);
    }
    if prefs.thunder {
        scan_prefixes(text, &["thunder://"], thunder_url, &mut matches);
    }
    if prefs.bt_infohash {
        matches.extend(extract_infohashes(text));
    }
    if prefs.metalink {
        scan_prefixes(
            text,
            &["metalink://", "meta4://"],
            |s| s.to_string(),
            &mut matches,
        );
    }

    matches.sort_by(|a, b| a.start.cmp(&b.start).then(b.end.cmp(&a.end)));

    let mut kept: Vec<LinkMatch> = Vec::new();
    for m in matches {
        if let Some(last) = kept.last() {
            if m.start < last.end {
                continue;
            }
        }
        kept.push(m);
    }

    let mut seen = HashSet::new();
    kept.into_iter()
        .filter_map(|m| {
            if m.value.is_empty() || !seen.insert(m.value.clone()) {
                return None;
            }
            Some(m.value)
        })
        .collect()
}

fn scan_prefixes(
    text: &str,
    prefixes: &[&str],
    transform: impl Fn(&str) -> String,
    out: &mut Vec<LinkMatch>,
) {
    for prefix in prefixes {
        let mut search_from = 0;
        while let Some(rel) = text[search_from..].find(prefix) {
            let start = search_from + rel;
            let end = token_end(text, start);
            let token = trim_trailing_punct(&text[start..end]);
            if !token.is_empty() {
                out.push(LinkMatch {
                    start,
                    end: start + token.len(),
                    value: transform(token),
                });
            }
            search_from = start + prefix.len();
        }
    }
}

fn token_end(text: &str, start: usize) -> usize {
    let mut end = start;
    for (i, ch) in text[start..].char_indices() {
        if is_link_terminator(ch) {
            break;
        }
        end = start + i + ch.len_utf8();
    }
    end
}

fn is_link_terminator(ch: char) -> bool {
    ch.is_whitespace()
        || matches!(ch, '"' | '\'' | '<' | '>' | ')' | ']' | '}')
        || matches!(
            ch,
            '，' | '。' | '；' | '：' | '！' | '？' | '、' | '）' | '】'
        )
        || ('\u{4e00}'..='\u{9fff}').contains(&ch)
}

fn trim_trailing_punct(mut s: &str) -> &str {
    while let Some(ch) = s.chars().next_back() {
        if matches!(
            ch,
            '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '}' | '>' | '"' | '\''
        ) {
            s = &s[..s.len() - ch.len_utf8()];
        } else {
            break;
        }
    }
    s
}

fn extract_infohashes(text: &str) -> Vec<LinkMatch> {
    let bytes = text.as_bytes();
    let n = bytes.len();
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        if bytes[i] == b'b' && text[i..].starts_with("btih:") {
            let hash_start = i + 5;
            if let Some(end) = infohash_after(text, hash_start) {
                out.push(LinkMatch {
                    start: i,
                    end,
                    value: format!("magnet:?xt=urn:btih:{}", &text[hash_start..end]),
                });
                i = end;
                continue;
            }
        }
        let b = bytes[i];
        if is_hex(b) && hex_run(text, i) == 40 && is_word_boundary(text, i, 40) {
            out.push(LinkMatch {
                start: i,
                end: i + 40,
                value: format!("magnet:?xt=urn:btih:{}", &text[i..i + 40]),
            });
            i += 40;
            continue;
        }
        if is_b32(b) && b32_run(text, i) == 32 && is_word_boundary(text, i, 32) {
            out.push(LinkMatch {
                start: i,
                end: i + 32,
                value: format!("magnet:?xt=urn:btih:{}", &text[i..i + 32]),
            });
            i += 32;
            continue;
        }
        i += 1;
    }
    out
}

fn infohash_after(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    if start < bytes.len()
        && is_hex(bytes[start])
        && hex_run(text, start) == 40
        && is_word_boundary(text, start, 40)
    {
        return Some(start + 40);
    }
    if start < bytes.len()
        && is_b32(bytes[start])
        && b32_run(text, start) == 32
        && is_word_boundary(text, start, 32)
    {
        return Some(start + 32);
    }
    None
}

fn hex_run(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut i = start;
    while i < bytes.len() && is_hex(bytes[i]) {
        i += 1;
    }
    i - start
}

fn b32_run(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut i = start;
    while i < bytes.len() && is_b32(bytes[i]) {
        i += 1;
    }
    i - start
}

fn is_hex(b: u8) -> bool {
    b.is_ascii_hexdigit()
}

fn is_b32(b: u8) -> bool {
    (b'2'..=b'7').contains(&b) || b.is_ascii_uppercase() || b.is_ascii_lowercase()
}

fn is_word_boundary(text: &str, start: usize, len: usize) -> bool {
    let bytes = text.as_bytes();
    if start > 0 && is_word_char(bytes[start - 1]) {
        return false;
    }
    if start + len < bytes.len() && is_word_char(bytes[start + len]) {
        return false;
    }
    true
}

fn is_word_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn thunder_url(token: &str) -> String {
    let Some(payload) = token.strip_prefix("thunder://") else {
        return token.to_string();
    };
    match base64::engine::general_purpose::STANDARD.decode(payload) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(s) => {
                let s = s.strip_prefix("AA").unwrap_or(&s);
                let s = s.strip_suffix("ZZ").unwrap_or(s);
                if s.starts_with("http://") || s.starts_with("https://") || s.starts_with("ftp://")
                {
                    s.to_string()
                } else {
                    token.to_string()
                }
            }
            Err(_) => token.to_string(),
        },
        Err(_) => token.to_string(),
    }
}

fn file_path_from_line(line: &str) -> Option<(PathBuf, u64)> {
    let line = line.trim();
    let decoded = line
        .strip_prefix("file://localhost")
        .or_else(|| line.strip_prefix("file://"))
        .map(percent_decode);
    let path_str = decoded.as_deref().unwrap_or(line);
    let path = PathBuf::from(path_str);
    let meta = std::fs::metadata(&path).ok()?;
    if meta.is_file() {
        Some((path, meta.len()))
    } else {
        None
    }
}

fn is_torrent_path(path: &Path) -> bool {
    path.extension()
        .map(|e| e.eq_ignore_ascii_case("torrent"))
        .unwrap_or(false)
}

fn file_content_links(
    path: &Path,
    len: u64,
    prefs: ClipboardLinkTypes,
) -> Option<ClipboardPayload> {
    if len == 0 || len > MAX_CLIPBOARD_CONTENT {
        return None;
    }
    let file = std::fs::File::open(path).ok()?;
    let mut bytes = Vec::with_capacity(len as usize);
    file.take(MAX_CLIPBOARD_CONTENT + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > MAX_CLIPBOARD_CONTENT {
        return None;
    }
    let content = String::from_utf8(bytes).ok()?;
    let urls = extract_links(&content, prefs);
    if urls.is_empty() {
        None
    } else {
        Some(ClipboardPayload::Urls(urls))
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(decoded) = hex::decode(&bytes[i + 1..i + 3]) {
                if let Some(&b) = decoded.first() {
                    out.push(b);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Lowercased file extension extracted from the last path segment, with
/// common compound archive suffixes (`tar.gz`, `tar.bz2`, `tar.xz`) kept
/// intact so the whitelist below matches them as one token.
fn file_extension(url_path: &str) -> Option<String> {
    let path = Path::new(url_path.trim_start_matches('/'));
    let filename = path.file_name()?.to_str()?;
    let lower = filename.to_ascii_lowercase();
    for compound in ["tar.gz", "tar.bz2", "tar.xz"] {
        if lower.ends_with(&format!(".{compound}")) {
            return Some(compound.to_string());
        }
    }
    Some(path.extension()?.to_str()?.to_ascii_lowercase())
}

fn is_download_extension(ext: &str) -> bool {
    matches!(
        ext,
        "zip"
            | "rar"
            | "7z"
            | "tar"
            | "tar.gz"
            | "tar.bz2"
            | "tar.xz"
            | "gz"
            | "tgz"
            | "bz2"
            | "xz"
            | "iso"
            | "img"
            | "dmg"
            | "deb"
            | "rpm"
            | "apk"
            | "msi"
            | "exe"
            | "appimage"
            | "run"
            | "pkg"
            | "pdf"
            | "epub"
            | "mobi"
            | "azw3"
            | "djvu"
            | "mp3"
            | "mp4"
            | "m4a"
            | "m4v"
            | "mkv"
            | "avi"
            | "wmv"
            | "flac"
            | "ogg"
            | "oga"
            | "opus"
            | "webm"
            | "flv"
            | "mov"
            | "wav"
            | "aac"
            | "doc"
            | "docx"
            | "xls"
            | "xlsx"
            | "ppt"
            | "pptx"
            | "csv"
            | "ts"
            | "m3u8"
    )
}

fn is_webpage_extension(ext: &str) -> bool {
    matches!(ext, "html" | "htm")
}

const DOWNLOAD_QUERY_KEYS: &[&str] = &["download", "filename", "file", "dl", "attachment"];

fn query_has_download_param(url: &reqwest::Url) -> bool {
    for (key, _) in url.query_pairs() {
        let key = key.to_ascii_lowercase();
        if DOWNLOAD_QUERY_KEYS.iter().any(|k| k == &key.as_str()) {
            return true;
        }
    }
    false
}

/// Cheap, offline pre-filter used by [`looks_like_download`]. Returns
/// `true` when the URL itself screams "download" (a known binary
/// extension or a query parameter like `?download=1`), `false` for
/// extension-less URLs that need a HEAD probe and for obvious HTML pages.
pub fn looks_like_download(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    if query_has_download_param(&parsed) {
        return true;
    }
    let Some(ext) = file_extension(parsed.path()) else {
        return false;
    };
    if is_webpage_extension(&ext) {
        return false;
    }
    is_download_extension(&ext)
}

/// Browser-style classification of an HTTP response's content type and
/// content disposition. Pure / sync so it can be unit-tested without
/// network access.
///
/// `disposition` wins outright when it contains `attachment` or
/// `filename=`, including the `form-data; name=...` case seen on
/// multipart responses, because the server explicitly labelled the
/// payload as a file the browser should save. Otherwise we fall back to
/// inspecting `content_type`:
/// - `text/html` / `application/xhtml+xml` → `Webpage`
/// - `application/octet-stream` and other archive / document / media
///   types → `Download`
/// - Inlinable types like `image/*` (which browsers render inline) fall
///   through to `Ambiguous` rather than `Download`; the Smart filter
///   keeps ambiguous URLs conservatively.
/// - `None` or `text/plain` → `Ambiguous` (could be either)
pub fn classify_content_type(ct: Option<&str>, disposition: Option<&str>) -> ContentDecision {
    if let Some(disp) = disposition {
        let lower = disp.to_ascii_lowercase();
        if lower.contains("attachment") || lower.contains("filename=") {
            return ContentDecision::Download;
        }
    }
    let Some(ct) = ct else {
        return ContentDecision::Ambiguous;
    };
    let ct = ct
        .split(';')
        .next()
        .unwrap_or(ct)
        .trim()
        .to_ascii_lowercase();
    if ct == "text/html" || ct == "application/xhtml+xml" {
        return ContentDecision::Webpage;
    }
    if ct == "application/octet-stream"
        || ct.starts_with("application/zip")
        || ct.starts_with("application/x-zip")
        || ct.starts_with("application/x-rar")
        || ct.starts_with("application/vnd.")
        || ct.starts_with("application/pdf")
        || ct.starts_with("application/epub")
        || ct == "application/json"
        || ct.starts_with("application/gzip")
        || ct.starts_with("application/x-gzip")
        || ct.starts_with("application/x-tar")
        || ct.starts_with("application/x-bzip2")
        || ct.starts_with("application/x-7z")
        || ct.starts_with("application/x-msi")
        || ct.starts_with("application/x-deb")
        || ct.starts_with("application/x-rpm")
        || ct.starts_with("video/")
        || ct.starts_with("audio/")
        || ct.starts_with("application/ogg")
    {
        return ContentDecision::Download;
    }
    ContentDecision::Ambiguous
}

fn classify_from_headers(resp: &reqwest::Response) -> ContentDecision {
    let ct = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok());
    let disp = resp
        .headers()
        .get(reqwest::header::CONTENT_DISPOSITION)
        .and_then(|v| v.to_str().ok());
    classify_content_type(ct, disp)
}

/// Probe a single URL with HEAD (falling back to a 1-byte ranged GET when
/// the server rejects HEAD) and classify the response. The response body
/// is intentionally never read: we drop the `Response` after inspecting
/// its headers so a misconfigured server cannot stream a multi-megabyte
/// HTML page into our process. Any error (timeout, non-2xx, transport
/// failure) collapses to `Ambiguous` — better to over-offer than to
/// silently drop a real download.
///
/// Times out at 5 s per leg (HEAD and the GET fallback are independent).
/// If a URL needs both legs (HEAD returns 405/501/403), the worst case for
/// that single URL is ~10 s. The set of probed URLs is dispatched in
/// parallel with `buffer_unordered(8)`, so a clipboard with N ambiguous
/// URLs is bounded by the slowest single URL, not by N × 5 s.
pub async fn probe_url(client: &reqwest::Client, url: &str) -> ContentDecision {
    match client.head(url).send().await {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() {
                let decision = classify_from_headers(&resp);
                tracing::debug!(%url, status = status.as_u16(), ?decision, "clipboard: HEAD probe ok");
                return decision;
            }
            let code = status.as_u16();
            if code != 405 && code != 501 && code != 403 {
                tracing::debug!(%url, status = code, "clipboard: HEAD non-2xx, ambiguous");
                return ContentDecision::Ambiguous;
            }
            tracing::debug!(%url, head_status = code, "clipboard: HEAD unsupported, falling back to ranged GET");
        }
        Err(e) => {
            if e.is_timeout() {
                tracing::warn!(%url, phase = "HEAD", "clipboard: HEAD timed out");
            } else {
                tracing::debug!(%url, phase = "HEAD", error = %e, "clipboard: HEAD transport error");
            }
        }
    }
    match client.get(url).header("Range", "bytes=0-0").send().await {
        Ok(resp) if resp.status().is_success() => {
            let decision = classify_from_headers(&resp);
            tracing::debug!(%url, status = resp.status().as_u16(), ?decision, "clipboard: ranged GET probe ok");
            decision
        }
        Ok(resp) => {
            tracing::debug!(%url, status = resp.status().as_u16(), "clipboard: ranged GET non-2xx, ambiguous");
            ContentDecision::Ambiguous
        }
        Err(e) => {
            if e.is_timeout() {
                tracing::warn!(%url, phase = "GET", "clipboard: ranged GET timed out");
            } else {
                tracing::debug!(%url, phase = "GET", error = %e, "clipboard: ranged GET transport error");
            }
            ContentDecision::Ambiguous
        }
    }
}

/// Apply [`WebpageFilterMode`] to a list of `http`/`https` URLs.
///
/// - `Off` returns the input unchanged.
/// - `Static` keeps only URLs whose [`looks_like_download`] is true.
/// - `Smart` keeps URL that look like downloads outright; for everything
///   else it probes the URL in parallel via [`probe_url`] and drops
///   those that resolve to `ContentDecision::Webpage`. Network failures
///   and `Ambiguous` responses are kept (conservative — never silently
///   discard a real download).
///
/// The function is async only because `Smart` performs network I/O;
/// `Off` and `Static` paths do not touch the network.
pub async fn filter_webpage_urls(
    urls: Vec<String>,
    mode: WebpageFilterMode,
    client: &reqwest::Client,
) -> Vec<String> {
    match mode {
        WebpageFilterMode::Off => urls,
        WebpageFilterMode::Static => {
            let total = urls.len();
            let kept: Vec<String> = urls
                .into_iter()
                .filter(|u| looks_like_download(u))
                .collect();
            tracing::debug!(
                input = total,
                kept = kept.len(),
                "clipboard: Static filter done"
            );
            kept
        }
        WebpageFilterMode::Smart => {
            let is_download: Vec<bool> = urls.iter().map(|u| looks_like_download(u)).collect();
            let prefilter_kept = is_download.iter().filter(|&&d| d).count();
            let probe_inputs: Vec<String> = urls
                .iter()
                .zip(&is_download)
                .filter(|(_, &d)| !d)
                .map(|(u, _)| u.clone())
                .collect();
            let decisions = probe_parallel(client, &probe_inputs).await;
            let mut decision_iter = decisions.into_iter();
            let mut kept = Vec::with_capacity(urls.len());
            let mut dropped = 0usize;
            for (url, is_dl) in urls.into_iter().zip(is_download) {
                if is_dl {
                    kept.push(url);
                } else {
                    match decision_iter.next() {
                        Some(ContentDecision::Webpage) => dropped += 1,
                        _ => kept.push(url),
                    }
                }
            }
            tracing::debug!(
                input = kept.len() + dropped,
                prefilter_kept,
                probed = probe_inputs.len(),
                kept = kept.len(),
                dropped,
                "clipboard: Smart filter done"
            );
            kept
        }
    }
}

async fn probe_parallel(client: &reqwest::Client, urls: &[String]) -> Vec<ContentDecision> {
    use futures::stream::{self, StreamExt};
    stream::iter(urls.iter().cloned())
        .map(|url| {
            let client = client.clone();
            async move { probe_url(&client, &url).await }
        })
        .buffer_unordered(8)
        .collect()
        .await
}

/// Build a short-lived `reqwest::Client` for clipboard URL probes, with
/// the user-configured proxy and user-agent applied and a 5-second
/// per-request timeout so a stuck server can't stall the watcher. With
/// HEAD-and-fallback-GET both at 5 s the worst-case per URL is ~10 s,
/// matching the original "≤10 s per clipboard batch" budget.
fn build_probe_client(proxy: Option<&str>, user_agent: &str) -> Result<reqwest::Client, String> {
    crate::config::apply_proxy(
        reqwest::Client::builder()
            .user_agent(user_agent)
            .timeout(Duration::from_secs(5)),
        proxy,
    )?
    .build()
    .map_err(|e| format!("build probe client: {e}"))
}

/// Apply the user's [`WebpageFilterMode`] to a parsed clipboard payload,
/// partitioning the URLs so that only `http`/`https` entries are
/// inspected (magnet / ed2k / ftp / bt_infohash / metalink are always
/// preserved). Returns `None` when filtering removes every URL, so the
/// watcher treats the clipboard as "nothing to download" and short-
/// circuits the dialog.
///
/// `Off` skips probe-client construction entirely. Any error building
/// the client (invalid proxy URL, etc.) collapses to "keep everything"
/// so a configuration mistake doesn't suppress real downloads.
///
/// # Recognition flow
///
/// 1. Partition the parsed URLs into `http`/`https` (filtered) and
///    everything else (always kept: `magnet:`, `ed2k:`, `ftp:`, bare
///    info-hash, `metalink:`).
/// 2. If the partition is empty or mode is `Off`, return the input
///    unchanged — no probe client is even constructed.
/// 3. `Static`: drop every URL for which `looks_like_download` is false.
///    Pure offline match; sub-millisecond.
/// 4. `Smart`: split into "obviously-download" (`looks_like_download` = true,
///    kept without I/O) and "ambiguous" (probed via HEAD → 1-byte ranged GET
///    fallback). Probes run concurrently (`buffer_unordered(8)`); each URL
///    is bounded to a 5 s timeout per leg, worst-case ~10 s per URL.
///
/// Typical timings (LAN, healthy server):
/// - `Off`      : < 1 ms
/// - `Static`   : < 1 ms (no network)
/// - `Smart`, 1 ambiguous URL, fast server: ~100–300 ms (one HEAD round-trip)
/// - `Smart`, N ambiguous URLs, parallelised 8-wide: roughly one HEAD RTT
///   plus a small constant; bounded by the slowest single URL (~5 s) on a
///   misbehaving server.
///
/// Failures collapse to `Ambiguous` (URL is kept, never silently dropped).
pub async fn apply_webpage_filter(
    payload: Option<ClipboardPayload>,
    mode: WebpageFilterMode,
    proxy: Option<&str>,
    user_agent: &str,
) -> Option<ClipboardPayload> {
    let payload = payload?;
    match payload {
        ClipboardPayload::Urls(urls) => {
            let started = Instant::now();
            let mut http_urls: Vec<String> = Vec::new();
            let mut other_urls: Vec<String> = Vec::new();
            for u in urls {
                if u.starts_with("http://") || u.starts_with("https://") {
                    http_urls.push(u);
                } else {
                    other_urls.push(u);
                }
            }
            tracing::debug!(
                ?mode,
                total = http_urls.len() + other_urls.len(),
                http = http_urls.len(),
                other = other_urls.len(),
                "clipboard: apply_webpage_filter start"
            );
            let http_in = http_urls.len();
            let filtered_http: Vec<String>;
            let dropped: usize;
            if http_urls.is_empty() || mode == WebpageFilterMode::Off {
                filtered_http = http_urls;
                dropped = 0;
            } else {
                match build_probe_client(proxy, user_agent) {
                    Ok(client) => {
                        filtered_http = filter_webpage_urls(http_urls, mode, &client).await;
                        dropped = http_in.saturating_sub(filtered_http.len());
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "clipboard: probe client build failed, keeping all URLs"
                        );
                        filtered_http = http_urls;
                        dropped = 0;
                    }
                }
            }
            let combined: Vec<String> = other_urls.into_iter().chain(filtered_http).collect();
            let kept = combined.len();
            let result = if combined.is_empty() {
                None
            } else {
                Some(ClipboardPayload::Urls(combined))
            };
            tracing::info!(
                ?mode,
                kept,
                dropped,
                elapsed_ms = started.elapsed().as_millis() as u64,
                "clipboard: filter done"
            );
            result
        }
        other => Some(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prefs() -> ClipboardLinkTypes {
        ClipboardLinkTypes::default()
    }

    fn urls_of(p: Option<ClipboardPayload>) -> Vec<String> {
        match p {
            Some(ClipboardPayload::Urls(urls)) => urls,
            other => panic!("expected Urls, got {other:?}"),
        }
    }

    #[test]
    fn extracts_links_from_mixed_text() {
        let p = parse_clipboard(
            "这是链接 ftp://mirror.example.com/a.iso 后面的文字 http://example.com/b.iso",
            prefs(),
        );
        assert_eq!(
            urls_of(p),
            vec!["ftp://mirror.example.com/a.iso", "http://example.com/b.iso"]
        );
    }

    #[test]
    fn magnet_with_embedded_hash_yields_single_link() {
        let hash = "0123456789abcdef0123456789abcdef01234567";
        let text = format!("magnet:?xt=urn:btih:{hash}&dn=test");
        let p = parse_clipboard(&text, prefs());
        assert_eq!(p, Some(ClipboardPayload::Urls(vec![text.clone()])));
    }

    #[test]
    fn thunder_link_is_decoded() {
        let p = parse_clipboard(
            "thunder://QUFodHRwOi8vZXhhbXBsZS5jb20vZi56aXBaWg==",
            prefs(),
        );
        assert_eq!(urls_of(p), vec!["http://example.com/f.zip"]);
    }

    #[test]
    fn thunder_invalid_base64_falls_back() {
        let link = "thunder://@@@@";
        let p = parse_clipboard(link, prefs());
        assert_eq!(p, Some(ClipboardPayload::Urls(vec![link.to_string()])));
    }

    #[test]
    fn thunder_non_url_decoded_falls_back() {
        let link = "thunder://aGVsbG8=";
        let p = parse_clipboard(link, prefs());
        assert_eq!(p, Some(ClipboardPayload::Urls(vec![link.to_string()])));
    }

    #[test]
    fn bare_hex_infohash_becomes_magnet() {
        let hash = "0123456789abcdef0123456789abcdef01234567";
        let p = parse_clipboard(hash, prefs());
        assert_eq!(
            p,
            Some(ClipboardPayload::Urls(vec![format!(
                "magnet:?xt=urn:btih:{hash}"
            )]))
        );
    }

    #[test]
    fn bare_base32_infohash_becomes_magnet() {
        let hash = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
        let p = parse_clipboard(hash, prefs());
        assert_eq!(
            p,
            Some(ClipboardPayload::Urls(vec![format!(
                "magnet:?xt=urn:btih:{hash}"
            )]))
        );
    }

    #[test]
    fn btih_prefix_converted() {
        let hash = "0123456789abcdef0123456789abcdef01234567";
        let p = parse_clipboard(&format!("btih:{hash}"), prefs());
        assert_eq!(
            p,
            Some(ClipboardPayload::Urls(vec![format!(
                "magnet:?xt=urn:btih:{hash}"
            )]))
        );
    }

    #[test]
    fn trailing_punctuation_is_stripped() {
        let p = parse_clipboard(
            "Download at http://example.com/a.zip. (mirror: https://example.com/b.zip)。",
            prefs(),
        );
        assert_eq!(
            urls_of(p),
            vec!["http://example.com/a.zip", "https://example.com/b.zip"]
        );
    }

    #[test]
    fn ed2k_link_extracted() {
        let link = "ed2k://|file|ubuntu.iso|123456|hash|/";
        let p = parse_clipboard(link, prefs());
        assert_eq!(p, Some(ClipboardPayload::Urls(vec![link.to_string()])));
    }

    #[test]
    fn disabled_type_is_not_extracted() {
        let mut p = prefs();
        p.ftp = false;
        assert_eq!(parse_clipboard("ftp://mirror.example.com/a.iso", p), None);
    }

    #[test]
    fn all_types_disabled_yields_none() {
        let p = ClipboardLinkTypes {
            http: false,
            ftp: false,
            magnet: false,
            ed2k: false,
            thunder: false,
            bt_infohash: false,
            metalink: false,
        };
        assert_eq!(
            parse_clipboard("http://example.com/a.iso magnet:?xt=urn:btih:abc", p),
            None
        );
    }

    #[test]
    fn non_link_text_returns_none() {
        assert_eq!(parse_clipboard("随便写点什么", prefs()), None);
    }

    #[test]
    fn torrent_path_still_recognized() {
        let dir = std::env::temp_dir().join(format!("remotrix-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.torrent");
        std::fs::write(&path, b"d4:infod4:name5:helloee").unwrap();
        let p = parse_clipboard(&path.to_string_lossy(), prefs());
        assert_eq!(p, Some(ClipboardPayload::Torrent(path)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("remotrix-test-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn oversized_text_is_ignored() {
        let mut text = String::new();
        while (text.len() as u64) <= MAX_CLIPBOARD_CONTENT {
            text.push_str("http://example.com/a.iso\n");
        }
        assert_eq!(parse_clipboard(&text, prefs()), None);
    }

    #[test]
    fn small_txt_file_bare_path_yields_urls() {
        let dir = temp_dir("txt_bare");
        let path = dir.join("links.txt");
        std::fs::write(&path, "Download from http://example.com/a.iso").unwrap();
        let p = parse_clipboard(&path.to_string_lossy(), prefs());
        assert_eq!(
            p,
            Some(ClipboardPayload::Urls(vec![
                "http://example.com/a.iso".to_string()
            ]))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn small_txt_file_uri_yields_urls() {
        let dir = temp_dir("txt_uri");
        let path = dir.join("links.txt");
        std::fs::write(&path, "https://example.com/b.iso").unwrap();
        let uri = format!("file://{}", path.to_string_lossy());
        let p = parse_clipboard(&uri, prefs());
        assert_eq!(
            p,
            Some(ClipboardPayload::Urls(vec![
                "https://example.com/b.iso".to_string()
            ]))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn oversized_file_is_ignored() {
        let dir = temp_dir("big_file");
        let path = dir.join("big.txt");
        let mut content = String::new();
        while (content.len() as u64) <= MAX_CLIPBOARD_CONTENT {
            content.push_str("http://example.com/a.iso\n");
        }
        std::fs::write(&path, content).unwrap();
        assert_eq!(parse_clipboard(&path.to_string_lossy(), prefs()), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn binary_file_is_ignored() {
        let dir = temp_dir("bin");
        let path = dir.join("data.bin");
        std::fs::write(&path, [0xffu8, 0x00, 0xfe, 0x01, b'x']).unwrap();
        assert_eq!(parse_clipboard(&path.to_string_lossy(), prefs()), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn payload_hash_is_stable_and_distinct() {
        let none = None;
        assert_eq!(payload_hash(&none), "");
        let urls = Some(ClipboardPayload::Urls(vec![
            "http://example.com/a.iso".to_string()
        ]));
        let hash1 = payload_hash(&urls);
        assert_eq!(payload_hash(&urls), hash1);
        assert_ne!(hash1, "");
        let torrent_a = Some(ClipboardPayload::Torrent(PathBuf::from("/tmp/a.torrent")));
        let torrent_b = Some(ClipboardPayload::Torrent(PathBuf::from("/tmp/b.torrent")));
        assert_ne!(hash1, payload_hash(&torrent_a));
        assert_ne!(payload_hash(&torrent_a), payload_hash(&torrent_b));
    }

    #[test]
    fn looks_like_download_matches_known_extensions() {
        assert!(looks_like_download("http://example.com/file.zip"));
        assert!(looks_like_download("https://example.com/path/ubuntu.iso"));
        assert!(looks_like_download("http://example.com/x.tar.gz"));
        assert!(looks_like_download("http://example.com/x.TAR.GZ"));
        assert!(looks_like_download("https://example.com/song.mp3"));
        assert!(looks_like_download("http://example.com/app.AppImage"));
    }

    #[test]
    fn looks_like_download_matches_query_params() {
        assert!(looks_like_download(
            "http://example.com/page?id=1&download=true"
        ));
        assert!(looks_like_download(
            "http://example.com/page?filename=foo.zip"
        ));
        assert!(looks_like_download("http://example.com/?dl=1"));
        assert!(looks_like_download("http://example.com/?file=foo"));
        assert!(looks_like_download("http://example.com/?attachment=1"));
        assert!(looks_like_download("http://example.com/?DOWNLOAD=1"));
    }

    #[test]
    fn looks_like_download_rejects_webpages() {
        assert!(!looks_like_download("http://example.com/index.html"));
        assert!(!looks_like_download("https://example.com/about.HTM"));
        assert!(!looks_like_download("http://example.com/"));
        assert!(!looks_like_download("http://example.com"));
        assert!(!looks_like_download("not a url"));
        assert!(!looks_like_download(""));
    }

    #[test]
    fn classify_content_type_disposition_attachment_is_download() {
        assert_eq!(
            classify_content_type(Some("text/html"), Some("attachment; filename=foo.zip")),
            ContentDecision::Download
        );
        assert_eq!(
            classify_content_type(None, Some("attachment")),
            ContentDecision::Download
        );
        assert_eq!(
            classify_content_type(None, Some("inline; filename=foo.bin")),
            ContentDecision::Download
        );
        assert_eq!(
            classify_content_type(None, Some("form-data; name=\"field\"; filename=\"x.zip\"")),
            ContentDecision::Download
        );
    }

    #[test]
    fn classify_content_type_html_is_webpage() {
        assert_eq!(
            classify_content_type(Some("text/html"), None),
            ContentDecision::Webpage
        );
        assert_eq!(
            classify_content_type(Some("text/html; charset=utf-8"), None),
            ContentDecision::Webpage
        );
        assert_eq!(
            classify_content_type(Some("application/xhtml+xml"), None),
            ContentDecision::Webpage
        );
    }

    #[test]
    fn classify_content_type_archives_and_media_are_download() {
        assert_eq!(
            classify_content_type(Some("application/octet-stream"), None),
            ContentDecision::Download
        );
        assert_eq!(
            classify_content_type(Some("application/zip"), None),
            ContentDecision::Download
        );
        assert_eq!(
            classify_content_type(Some("application/vnd.android.package-archive"), None),
            ContentDecision::Download
        );
        assert_eq!(
            classify_content_type(Some("application/pdf"), None),
            ContentDecision::Download
        );
        assert_eq!(
            classify_content_type(Some("video/mp4"), None),
            ContentDecision::Download
        );
        assert_eq!(
            classify_content_type(Some("audio/mpeg"), None),
            ContentDecision::Download
        );
    }

    #[test]
    fn classify_content_type_unknown_or_plain_is_ambiguous() {
        assert_eq!(
            classify_content_type(None, None),
            ContentDecision::Ambiguous
        );
        assert_eq!(
            classify_content_type(Some("text/plain"), None),
            ContentDecision::Ambiguous
        );
        assert_eq!(
            classify_content_type(Some("application/x-fantasy-mime"), None),
            ContentDecision::Ambiguous
        );
    }

    #[tokio::test]
    async fn filter_webpage_urls_off_is_passthrough() {
        let client = reqwest::Client::new();
        let urls = vec![
            "http://example.com/".to_string(),
            "https://example.com/index.html".to_string(),
            "http://example.com/no-extension".to_string(),
        ];
        let kept = filter_webpage_urls(urls.clone(), WebpageFilterMode::Off, &client).await;
        assert_eq!(kept, urls);
    }

    #[tokio::test]
    async fn filter_webpage_urls_static_drops_ambiguous() {
        let client = reqwest::Client::new();
        let urls = vec![
            "http://example.com/file.zip".to_string(),
            "http://example.com/no-extension".to_string(),
            "https://example.com/index.html".to_string(),
            "http://example.com/?download=1".to_string(),
        ];
        let kept = filter_webpage_urls(urls, WebpageFilterMode::Static, &client).await;
        assert_eq!(
            kept,
            vec![
                "http://example.com/file.zip".to_string(),
                "http://example.com/?download=1".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn filter_webpage_urls_smart_keeps_known_without_probing() {
        let client = reqwest::Client::new();
        let urls = vec![
            "http://example.com/file.zip".to_string(),
            "http://example.com/?download=1".to_string(),
            "https://example.com/x.tar.gz".to_string(),
        ];
        let kept = filter_webpage_urls(urls.clone(), WebpageFilterMode::Smart, &client).await;
        assert_eq!(kept, urls);
    }

    #[tokio::test]
    async fn apply_webpage_filter_preserves_non_http_schemes() {
        let payload = Some(ClipboardPayload::Urls(vec![
            "magnet:?xt=urn:btih:abcdefabcdefabcdefabcdefabcdefabcdefabcdef".to_string(),
            "http://example.com/file.zip".to_string(),
        ]));
        let out =
            apply_webpage_filter(payload, WebpageFilterMode::Static, None, "remotrix-test").await;
        match out.unwrap() {
            ClipboardPayload::Urls(urls) => {
                assert_eq!(urls.len(), 2);
                assert!(urls.iter().any(|u| u.starts_with("magnet:")));
                assert!(urls.iter().any(|u| u.contains("file.zip")));
            }
            _ => panic!("expected Urls"),
        }
    }

    #[tokio::test]
    async fn apply_webpage_filter_returns_none_when_all_filtered() {
        let payload = Some(ClipboardPayload::Urls(vec![
            "https://example.com/page.html".to_string(),
            "https://example.com/about.html".to_string(),
        ]));
        let out =
            apply_webpage_filter(payload, WebpageFilterMode::Static, None, "remotrix-test").await;
        assert_eq!(out, None);
    }

    #[tokio::test]
    async fn apply_webpage_filter_off_keeps_everything() {
        let payload = Some(ClipboardPayload::Urls(vec![
            "https://example.com/".to_string(),
            "https://example.com/page.html".to_string(),
        ]));
        let out =
            apply_webpage_filter(payload, WebpageFilterMode::Off, None, "remotrix-test").await;
        match out.unwrap() {
            ClipboardPayload::Urls(urls) => assert_eq!(urls.len(), 2),
            _ => panic!("expected Urls"),
        }
    }
}
