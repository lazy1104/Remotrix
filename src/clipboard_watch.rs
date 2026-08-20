//! Detects whether a clipboard payload (text or a pasted file path) should be
//! offered as a download. Used by the periodic clipboard watcher in `app.rs`
//! to decide whether a new payload is worth forwarding to the "Add download"
//! dialog. URLs are classified by scheme (http / ftp / magnet / ed2k /
//! thunder), and bare BitTorrent info-hashes are lifted into `magnet:` links.

use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};

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
}
