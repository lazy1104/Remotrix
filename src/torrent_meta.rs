#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TorrentFile {
    pub index: u64,
    pub path: String,
    pub length: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TorrentMeta {
    pub name: String,
    pub files: Vec<TorrentFile>,
}

const MAX_BENCODE_DEPTH: usize = 64;

pub fn parse_torrent(bytes: &[u8]) -> Option<TorrentMeta> {
    let mut parser = Parser::new(bytes);
    let root = parser.parse_value()?;
    let root_dict = root.as_dict()?;
    let info = lookup(root_dict, b"info")?.as_dict()?;

    let name = String::from_utf8_lossy(lookup(info, b"name")?.as_bytes()?).into_owned();

    let mut files: Vec<TorrentFile> = Vec::new();
    if let Some(length) = lookup(info, b"length").and_then(|v| v.as_int()) {
        if length < 0 {
            return None;
        }
        files.push(TorrentFile {
            index: 1,
            path: name.clone(),
            length: length as u64,
        });
    } else if let Some(list) = lookup(info, b"files").and_then(|v| v.as_list()) {
        for (i, entry) in list.iter().enumerate() {
            let entry = entry.as_dict()?;
            let length = lookup(entry, b"length")?.as_int()?;
            if length < 0 {
                return None;
            }
            let path = lookup(entry, b"path")?.as_list()?;
            let mut segments: Vec<String> = Vec::new();
            for seg in path {
                segments.push(String::from_utf8_lossy(seg.as_bytes()?).into_owned());
            }
            if segments.is_empty() {
                return None;
            }
            let rel = if name.is_empty() {
                segments.join("/")
            } else {
                format!("{}/{}", name, segments.join("/"))
            };
            files.push(TorrentFile {
                index: (i + 1) as u64,
                path: rel,
                length: length as u64,
            });
        }
    } else {
        return None;
    }

    if files.is_empty() {
        return None;
    }

    Some(TorrentMeta { name, files })
}

enum Value<'a> {
    Int(i64),
    Bytes(&'a [u8]),
    List(Vec<Value<'a>>),
    Dict(Vec<(Value<'a>, Value<'a>)>),
}

impl<'a> Value<'a> {
    fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(*n),
            _ => None,
        }
    }

    fn as_bytes(&self) -> Option<&'a [u8]> {
        match self {
            Value::Bytes(b) => Some(b),
            _ => None,
        }
    }

    fn as_list(&self) -> Option<&Vec<Value<'a>>> {
        match self {
            Value::List(items) => Some(items),
            _ => None,
        }
    }

    fn as_dict(&self) -> Option<&Vec<(Value<'a>, Value<'a>)>> {
        match self {
            Value::Dict(entries) => Some(entries),
            _ => None,
        }
    }
}

fn lookup<'a>(entries: &'a [(Value<'a>, Value<'a>)], key: &[u8]) -> Option<&'a Value<'a>> {
    entries
        .iter()
        .find(|(k, _)| k.as_bytes() == Some(key))
        .map(|(_, v)| v)
}

struct Parser<'a> {
    data: &'a [u8],
    pos: usize,
    depth: usize,
}

impl<'a> Parser<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            depth: 0,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.data.get(self.pos).copied()
    }

    fn parse_value(&mut self) -> Option<Value<'a>> {
        match self.peek()? {
            b'i' => self.parse_int(),
            b'l' => self.parse_list(),
            b'd' => self.parse_dict(),
            b'0'..=b'9' => self.parse_string(),
            _ => None,
        }
    }

    fn parse_int(&mut self) -> Option<Value<'a>> {
        if self.peek()? != b'i' {
            return None;
        }
        self.pos += 1;
        let start = self.pos;
        if self.peek()? == b'-' {
            self.pos += 1;
        }
        let digits_start = self.pos;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
        if digits_start == self.pos {
            return None;
        }
        if self.peek()? != b'e' {
            return None;
        }
        let text = std::str::from_utf8(&self.data[start..self.pos]).ok()?;
        self.pos += 1;
        Some(Value::Int(text.parse::<i64>().ok()?))
    }

    fn parse_string(&mut self) -> Option<Value<'a>> {
        let start = self.pos;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
        if start == self.pos {
            return None;
        }
        if self.peek()? != b':' {
            return None;
        }
        let len = std::str::from_utf8(&self.data[start..self.pos])
            .ok()?
            .parse::<usize>()
            .ok()?;
        self.pos += 1;
        let end = self.pos.checked_add(len)?;
        let bytes = self.data.get(self.pos..end)?;
        self.pos = end;
        Some(Value::Bytes(bytes))
    }

    fn parse_list(&mut self) -> Option<Value<'a>> {
        if self.peek()? != b'l' {
            return None;
        }
        self.pos += 1;
        self.depth += 1;
        if self.depth > MAX_BENCODE_DEPTH {
            return None;
        }
        let mut items = Vec::new();
        while self.peek()? != b'e' {
            items.push(self.parse_value()?);
        }
        self.pos += 1;
        self.depth -= 1;
        Some(Value::List(items))
    }

    fn parse_dict(&mut self) -> Option<Value<'a>> {
        if self.peek()? != b'd' {
            return None;
        }
        self.pos += 1;
        self.depth += 1;
        if self.depth > MAX_BENCODE_DEPTH {
            return None;
        }
        let mut entries = Vec::new();
        while self.peek()? != b'e' {
            let key = self.parse_string()?;
            let value = self.parse_value()?;
            entries.push((key, value));
        }
        self.pos += 1;
        self.depth -= 1;
        Some(Value::Dict(entries))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> Vec<u8> {
        format!("{}:{}", v.len(), v).into_bytes()
    }

    fn int(n: i64) -> Vec<u8> {
        format!("i{n}e").into_bytes()
    }

    fn list(items: &[Vec<u8>]) -> Vec<u8> {
        let mut out = vec![b'l'];
        for item in items {
            out.extend_from_slice(item);
        }
        out.push(b'e');
        out
    }

    fn dict(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
        let mut out = vec![b'd'];
        for (key, value) in entries {
            out.extend_from_slice(&s(key));
            out.extend_from_slice(value);
        }
        out.push(b'e');
        out
    }

    fn info_single(length: i64, name: &str) -> Vec<u8> {
        dict(&[("length", int(length)), ("name", s(name))])
    }

    fn info_multi(name: &str, file_paths: &[(&[&str], i64)]) -> Vec<u8> {
        let files: Vec<Vec<u8>> = file_paths
            .iter()
            .map(|(segments, len)| {
                let path: Vec<Vec<u8>> = segments.iter().map(|seg| s(seg)).collect();
                dict(&[("length", int(*len)), ("path", list(&path))])
            })
            .collect();
        dict(&[("name", s(name)), ("files", list(&files))])
    }

    #[test]
    fn parses_single_file() {
        let bytes = dict(&[("info", info_single(12345, "test.txt"))]);
        let meta = parse_torrent(&bytes).expect("parse");
        assert_eq!(meta.files.len(), 1);
        assert_eq!(meta.files[0].index, 1);
        assert_eq!(meta.files[0].path, "test.txt");
        assert_eq!(meta.files[0].length, 12345);
    }

    #[test]
    fn parses_multi_file_preserving_order() {
        let bytes = dict(&[(
            "info",
            info_multi(
                "bundle",
                &[
                    (&["dir-b", "second.bin"], 200),
                    (&["dir-a", "first.bin"], 100),
                    (&["top.txt"], 50),
                ],
            ),
        )]);
        let meta = parse_torrent(&bytes).expect("parse");
        assert_eq!(
            meta.files
                .iter()
                .map(|f| f.path.clone())
                .collect::<Vec<_>>(),
            vec![
                "bundle/dir-b/second.bin",
                "bundle/dir-a/first.bin",
                "bundle/top.txt",
            ]
        );
        assert_eq!(
            meta.files.iter().map(|f| f.index).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn rejects_negative_length() {
        let bytes = dict(&[("info", info_single(-5, "bad.txt"))]);
        assert!(parse_torrent(&bytes).is_none());
    }

    #[test]
    fn rejects_deeply_nested_input() {
        let mut bytes = Vec::new();
        for _ in 0..(MAX_BENCODE_DEPTH + 1) {
            bytes.push(b'l');
        }
        bytes.push(b'e');
        assert!(parse_torrent(&bytes).is_none());
    }

    #[test]
    fn rejects_truncated_or_garbage() {
        assert!(parse_torrent(b"").is_none());
        assert!(parse_torrent(b"d4:infod").is_none());
        assert!(parse_torrent(b"this is not bencode").is_none());
        assert!(parse_torrent(b"d4:infod3:fooe").is_none());
    }

    #[test]
    fn rejects_missing_length_and_files() {
        let bytes = dict(&[("info", dict(&[("name", s("nothing"))]))]);
        assert!(parse_torrent(&bytes).is_none());
    }
}
