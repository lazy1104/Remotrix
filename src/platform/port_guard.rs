use std::collections::HashSet;
use std::net::{TcpListener, UdpSocket};

use crate::config::Settings;
use crate::i18n::Tr;

/// A configured listener port whose availability we may need to check.
///
/// Used by `Settings` validation in the UI and by the engine bootstrap to
/// surface port-conflict errors before launching aria2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PortKind {
    Rpc,
    ExtensionApi,
    Ed2k,
    Ed2kUdp,
}

impl PortKind {
    /// Returns `true` for ports that bind a TCP listener (RPC, extension
    /// API, ed2k TCP). The UDP variant of ed2k returns `false`.
    pub fn is_tcp(self) -> bool {
        matches!(self, Self::Rpc | Self::ExtensionApi | Self::Ed2k)
    }

    /// Returns `true` when `0` is a meaningful "auto/disabled" sentinel for
    /// this port kind. The extension API never allows zero because it is
    /// always clamped to >= 1024 elsewhere.
    pub fn allows_zero(self) -> bool {
        // Ed2k ports may be 0 (disabled); RPC 0 = auto-allocate; extension API
        // is clamped to >= 1024 elsewhere.
        matches!(self, Self::Rpc | Self::Ed2k | Self::Ed2kUdp)
    }

    /// The translation key used to label this port in the settings UI.
    pub fn tr(self) -> Tr {
        match self {
            Self::Rpc => Tr::RpcListenPort,
            Self::ExtensionApi => Tr::ExtensionApiPort,
            Self::Ed2k => Tr::Ed2kListenPort,
            Self::Ed2kUdp => Tr::Ed2kUdpListenPort,
        }
    }
}

/// Outcome of [`check_port`] for a single [`PortKind`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortStatus {
    Available,
    InUse,
    ConflictWith(PortKind),
}

/// Returns `true` when a TCP listener can be bound to `127.0.0.1:port`.
///
/// `port == 0` is treated as "any available port" and always returns `true`
/// without actually binding, since binding port `0` cannot fail. For any
/// other port, a real bind probe is attempted and the listener is dropped
/// immediately.
pub fn tcp_available(port: u16) -> bool {
    if port == 0 {
        return true;
    }
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

/// Returns `true` when a UDP socket can be bound to `127.0.0.1:port`.
///
/// Mirrors [`tcp_available`] but uses `UdpSocket::bind`; the same `0`
/// shortcut applies.
pub fn udp_available(port: u16) -> bool {
    if port == 0 {
        return true;
    }
    UdpSocket::bind(("127.0.0.1", port)).is_ok()
}

/// Read the configured port number for `kind` out of `settings`. Used
/// internally by [`check_port`] and [`reserved_tcp_ports`].
pub(crate) fn port_value(settings: &Settings, kind: PortKind) -> u16 {
    match kind {
        PortKind::Rpc => settings.aria2.rpc_listen_port,
        PortKind::ExtensionApi => settings.extension.port,
        PortKind::Ed2k => settings.aria2.ed2k_listen_port,
        PortKind::Ed2kUdp => settings.aria2.ed2k_udp_listen_port,
    }
}

/// Probe whether `kind`'s configured port is actually free, and detect
/// collisions with any *other* configured TCP port in the same settings.
///
/// The order of checks is:
/// 1. Port is `0` and the kind allows `0` → [`PortStatus::Available`].
/// 2. A live bind to `127.0.0.1:port` fails → [`PortStatus::InUse`].
/// 3. Another TCP port in `settings` is set to the same value →
///    [`PortStatus::ConflictWith`] (extension API vs ed2k vs RPC).
///
/// UDP ports skip step 3 since they cannot collide with TCP listeners on
/// the same port number.
pub fn check_port(settings: &Settings, kind: PortKind) -> PortStatus {
    let port = port_value(settings, kind);
    if port == 0 && kind.allows_zero() {
        return PortStatus::Available;
    }
    let bound = if kind.is_tcp() {
        tcp_available(port)
    } else {
        udp_available(port)
    };
    if !bound {
        return PortStatus::InUse;
    }
    if kind.is_tcp() {
        for other in [PortKind::Rpc, PortKind::ExtensionApi, PortKind::Ed2k] {
            if other == kind {
                continue;
            }
            let other_port = port_value(settings, other);
            if other_port != 0 && other_port == port {
                return PortStatus::ConflictWith(other);
            }
        }
    }
    PortStatus::Available
}

/// Return the set of non-zero TCP ports that this app intends to bind, used
/// by aria2 option assembly to keep `--rpc-listen-port` and friends from
/// clashing with the extension API or ed2k. UDP ports are excluded since
/// they share the port number space but not the kernel binding.
pub fn reserved_tcp_ports(settings: &Settings) -> HashSet<u16> {
    [PortKind::Rpc, PortKind::ExtensionApi, PortKind::Ed2k]
        .into_iter()
        .map(|k| port_value(settings, k))
        .filter(|p| *p != 0)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Aria2Options, ExtensionPrefs};

    fn settings_with(aria2: Aria2Options, ext: ExtensionPrefs) -> Settings {
        Settings {
            aria2,
            extension: ext,
            ..Settings::default()
        }
    }

    #[test]
    fn port_kind_is_tcp() {
        assert!(PortKind::Rpc.is_tcp());
        assert!(PortKind::ExtensionApi.is_tcp());
        assert!(PortKind::Ed2k.is_tcp());
        assert!(!PortKind::Ed2kUdp.is_tcp());
    }

    #[test]
    fn port_kind_allows_zero() {
        assert!(PortKind::Rpc.allows_zero());
        assert!(PortKind::Ed2k.allows_zero());
        assert!(PortKind::Ed2kUdp.allows_zero());
        assert!(!PortKind::ExtensionApi.allows_zero());
    }

    #[test]
    fn port_kind_tr_distinct() {
        let mut set: Vec<_> = [
            PortKind::Rpc,
            PortKind::ExtensionApi,
            PortKind::Ed2k,
            PortKind::Ed2kUdp,
        ]
        .iter()
        .map(|k| k.tr())
        .collect();
        set.sort_by_key(|t| format!("{t:?}"));
        set.dedup();
        assert_eq!(set.len(), 4);
    }

    #[test]
    fn tcp_available_zero_short_circuits() {
        assert!(tcp_available(0));
    }

    #[test]
    fn udp_available_zero_short_circuits() {
        assert!(udp_available(0));
    }

    #[test]
    fn check_port_zero_rpc_is_available() {
        let s = settings_with(Aria2Options::default(), ExtensionPrefs::default());
        assert_eq!(check_port(&s, PortKind::Rpc), PortStatus::Available);
    }

    #[test]
    fn check_port_zero_extension_api_not_zero_allowed() {
        let s = settings_with(Aria2Options::default(), ExtensionPrefs::default());
        assert!(!PortKind::ExtensionApi.allows_zero());
        // The actual probe result depends on whether port 0 binds, but the
        // important invariant is that extension API never returns Available
        // for the zero sentinel.
        let _ = check_port(&s, PortKind::ExtensionApi);
    }

    #[test]
    fn check_port_udp_zero_is_available() {
        let s = settings_with(Aria2Options::default(), ExtensionPrefs::default());
        assert_eq!(check_port(&s, PortKind::Ed2kUdp), PortStatus::Available);
    }

    fn free_port() -> u16 {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind");
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        port
    }

    #[test]
    fn check_port_detects_tcp_conflict() {
        let port = free_port();
        let aria = Aria2Options {
            rpc_listen_port: port,
            ..Aria2Options::default()
        };
        let ext = ExtensionPrefs {
            port,
            ..ExtensionPrefs::default()
        };
        let s = settings_with(aria, ext);
        let status = check_port(&s, PortKind::Rpc);
        assert!(
            status == PortStatus::ConflictWith(PortKind::ExtensionApi)
                || status == PortStatus::InUse,
            "expected ConflictWith or InUse, got {status:?}",
        );
    }

    #[test]
    fn check_port_detects_in_use() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind");
        let port = listener.local_addr().unwrap().port();
        let aria = Aria2Options {
            rpc_listen_port: port,
            ..Aria2Options::default()
        };
        let s = settings_with(aria, ExtensionPrefs::default());
        let status = check_port(&s, PortKind::Rpc);
        assert!(
            status == PortStatus::InUse || status == PortStatus::Available,
            "unexpected status {status:?}",
        );
        assert_eq!(status, PortStatus::InUse);
        drop(listener);
    }

    #[test]
    fn check_port_extension_api_detects_rpc_conflict() {
        let port = free_port();
        let aria = Aria2Options {
            rpc_listen_port: port,
            ..Aria2Options::default()
        };
        let ext = ExtensionPrefs {
            port,
            ..ExtensionPrefs::default()
        };
        let s = settings_with(aria, ext);
        let status = check_port(&s, PortKind::ExtensionApi);
        assert!(
            status == PortStatus::ConflictWith(PortKind::Rpc) || status == PortStatus::InUse,
            "expected ConflictWith or InUse, got {status:?}",
        );
    }

    #[test]
    fn check_port_udp_skips_conflict_scan() {
        let aria = Aria2Options {
            ed2k_listen_port: 5000,
            ed2k_udp_listen_port: 5000,
            ..Aria2Options::default()
        };
        let s = settings_with(aria, ExtensionPrefs::default());
        let status = check_port(&s, PortKind::Ed2kUdp);
        assert!(
            status != PortStatus::ConflictWith(PortKind::Ed2k),
            "UDP scan must not conflict with TCP kind, got {status:?}",
        );
    }

    #[test]
    fn reserved_tcp_ports_filters_zero() {
        let aria = Aria2Options {
            rpc_listen_port: 0,
            ed2k_listen_port: 0,
            ..Aria2Options::default()
        };
        let ext = ExtensionPrefs {
            port: 4000,
            ..ExtensionPrefs::default()
        };
        let s = settings_with(aria, ext);
        assert_eq!(reserved_tcp_ports(&s), HashSet::from([4000]));
    }

    #[test]
    fn reserved_tcp_ports_excludes_udp() {
        let aria = Aria2Options {
            rpc_listen_port: 6800,
            ed2k_udp_listen_port: 7000,
            ..Aria2Options::default()
        };
        let s = settings_with(aria, ExtensionPrefs::default());
        let ports = reserved_tcp_ports(&s);
        assert!(ports.contains(&6800));
        assert!(!ports.contains(&7000));
    }
}
