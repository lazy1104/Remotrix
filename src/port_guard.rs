use std::collections::HashSet;
use std::net::{TcpListener, UdpSocket};

use crate::config::Settings;
use crate::i18n::Tr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PortKind {
    Rpc,
    ExtensionApi,
    Ed2k,
    Ed2kUdp,
}

impl PortKind {
    pub fn is_tcp(self) -> bool {
        matches!(self, Self::Rpc | Self::ExtensionApi | Self::Ed2k)
    }

    pub fn allows_zero(self) -> bool {
        // Ed2k ports may be 0 (disabled); RPC 0 = auto-allocate; extension API
        // is clamped to >= 1024 elsewhere.
        matches!(self, Self::Rpc | Self::Ed2k | Self::Ed2kUdp)
    }

    pub fn tr(self) -> Tr {
        match self {
            Self::Rpc => Tr::RpcListenPort,
            Self::ExtensionApi => Tr::ExtensionApiPort,
            Self::Ed2k => Tr::Ed2kListenPort,
            Self::Ed2kUdp => Tr::Ed2kUdpListenPort,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortStatus {
    Available,
    InUse,
    ConflictWith(PortKind),
}

pub fn tcp_available(port: u16) -> bool {
    if port == 0 {
        return true;
    }
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

pub fn udp_available(port: u16) -> bool {
    if port == 0 {
        return true;
    }
    UdpSocket::bind(("127.0.0.1", port)).is_ok()
}

pub(crate) fn port_value(settings: &Settings, kind: PortKind) -> u16 {
    match kind {
        PortKind::Rpc => settings.aria2.rpc_listen_port,
        PortKind::ExtensionApi => settings.extension.port,
        PortKind::Ed2k => settings.aria2.ed2k_listen_port,
        PortKind::Ed2kUdp => settings.aria2.ed2k_udp_listen_port,
    }
}

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

pub fn reserved_tcp_ports(settings: &Settings) -> HashSet<u16> {
    [PortKind::Rpc, PortKind::ExtensionApi, PortKind::Ed2k]
        .into_iter()
        .map(|k| port_value(settings, k))
        .filter(|p| *p != 0)
        .collect()
}
