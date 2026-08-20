//! Drop zone + file-picker widget for selecting a single `.torrent` file
//! in the Add dialog. Thin wrapper around the file-type-agnostic
//! [`FileDropZone`], passing torrent-specific hint text and re-exporting
//! the shared event / action enums under their historical names so
//! downstream call sites stay stable.

use std::ops::{Deref, DerefMut};
use std::path::Path;

use iced::Element;

use super::file_drop_zone::{FileDropAction, FileDropEvent, FileDropZone};
use crate::i18n::{Fluent, Tr};

/// Maximum `.torrent` file size, in bytes, that [`is_valid_torrent_file`]
/// will accept. The 50 MiB ceiling is a defensive guard against malicious
/// or accidentally gigantic files being loaded into memory during
/// validation; legitimate torrents are well under this bound.
pub const MAX_TORRENT_SIZE: u64 = 50 * 1024 * 1024;

/// Return `true` when `p` ends with a `.torrent` extension (ASCII-case
/// insensitive). Matches on the last extension component only, so paths
/// like `dir/.torrent/foo` are rejected.
///
/// Does not touch the filesystem; the parent decides whether to also call
/// [`is_valid_torrent_file`] before accepting the file.
pub fn is_torrent_file(p: &Path) -> bool {
    p.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase() == "torrent")
        .unwrap_or(false)
}

/// Validate that `p` is a reasonable torrent file before parsing it:
/// extension is `.torrent`, the file is non-empty, at most
/// [`MAX_TORRENT_SIZE`] bytes long, readable, and starts with the bencode
/// dictionary marker (`b'd'`). Any filesystem or read error causes the
/// function to return `false` rather than propagating the error.
pub fn is_valid_torrent_file(p: &Path) -> bool {
    if !is_torrent_file(p) {
        return false;
    }
    let Ok(meta) = std::fs::metadata(p) else {
        return false;
    };
    if meta.len() == 0 || meta.len() > MAX_TORRENT_SIZE {
        return false;
    }
    let Ok(mut f) = std::fs::File::open(p) else {
        return false;
    };
    let mut buf = [0u8; 1];
    use std::io::Read;
    if f.read_exact(&mut buf).is_err() {
        return false;
    }
    buf[0] == b'd'
}

/// Torrent-specific drop-zone view over the generic [`FileDropZone`].
/// Wraps the shared component so existing call sites keep their familiar
/// name while Metalink (and future) callers can use the same widget
/// directly with their own hint strings.
#[derive(Debug, Clone)]
pub struct TorrentUpload(FileDropZone);

#[allow(dead_code)]
pub type TorrentUploadEvent = FileDropEvent;
#[allow(dead_code)]
pub type TorrentUploadAction = FileDropAction;

impl Deref for TorrentUpload {
    type Target = FileDropZone;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for TorrentUpload {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Default for TorrentUpload {
    fn default() -> Self {
        Self::new()
    }
}

impl TorrentUpload {
    pub fn new() -> Self {
        Self(FileDropZone::new())
    }

    pub fn view<'a, M>(
        &'a self,
        fluent: &'a Fluent,
        theme: &'a iced::Theme,
        map: impl Fn(TorrentUploadEvent) -> M + 'a,
    ) -> Element<'a, M>
    where
        M: Clone + 'a,
    {
        self.0.view(
            fluent,
            theme,
            Tr::DropTorrentHint,
            Tr::DropTorrentActive,
            map,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn is_torrent_file_basic() {
        assert!(is_torrent_file(&PathBuf::from("foo.torrent")));
    }

    #[test]
    fn is_torrent_file_uppercase() {
        assert!(is_torrent_file(&PathBuf::from("FOO.TORRENT")));
        assert!(is_torrent_file(&PathBuf::from("Foo.Torrent")));
    }

    #[test]
    fn is_torrent_file_no_extension() {
        assert!(!is_torrent_file(&PathBuf::from("foo")));
    }

    #[test]
    fn is_torrent_file_other_ext() {
        assert!(!is_torrent_file(&PathBuf::from("foo.txt")));
        assert!(!is_torrent_file(&PathBuf::from("foo.metalink")));
        assert!(!is_torrent_file(&PathBuf::from("foo.meta4")));
    }

    #[test]
    fn is_torrent_file_in_path() {
        assert!(!is_torrent_file(&PathBuf::from("dir/.torrent/foo")));
    }

    #[test]
    fn wrapper_forwards_to_inner() {
        let mut zone = TorrentUpload::new();
        zone.set_path("/tmp/a.torrent");
        assert!(!zone.is_empty());
        assert_eq!(zone.path(), "/tmp/a.torrent");
        zone.clear();
        assert!(zone.is_empty());
    }
}
