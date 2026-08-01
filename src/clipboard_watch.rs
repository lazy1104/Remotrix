use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum ClipboardPayload {
    Urls(Vec<String>),
    Torrent(PathBuf),
}

pub fn parse_clipboard(text: &str) -> Option<ClipboardPayload> {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    match lines.len() {
        0 => None,
        1 => {
            let line = lines[0];
            if let Some(path) = torrent_path_from_line(line) {
                Some(ClipboardPayload::Torrent(path))
            } else if is_url(line) {
                Some(ClipboardPayload::Urls(vec![line.to_string()]))
            } else {
                None
            }
        }
        _ => {
            if lines.iter().all(|l| is_url(l)) {
                Some(ClipboardPayload::Urls(
                    lines.iter().map(|l| l.to_string()).collect(),
                ))
            } else if is_url(lines[0]) {
                Some(ClipboardPayload::Urls(vec![lines[0].to_string()]))
            } else {
                None
            }
        }
    }
}

fn is_url(line: &str) -> bool {
    let line = line.trim();
    line.starts_with("magnet:?") || line.contains("://")
}

fn torrent_path_from_line(line: &str) -> Option<PathBuf> {
    let line = line.trim();
    let decoded = line
        .strip_prefix("file://localhost")
        .or_else(|| line.strip_prefix("file://"))
        .map(percent_decode);
    let path_str = decoded.as_deref().unwrap_or(line);
    let path = PathBuf::from(path_str);
    let is_torrent = path
        .extension()
        .map(|e| e.eq_ignore_ascii_case("torrent"))
        .unwrap_or(false);
    if is_torrent && path.is_file() {
        Some(path)
    } else {
        None
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
