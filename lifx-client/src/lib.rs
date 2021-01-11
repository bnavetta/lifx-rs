use std::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use lifx_proto::wire::DeviceTarget;

mod any_message;
mod client;
mod codec;
mod connection;
mod error;

pub use any_message::AnyMessage;
pub use client::Client;
pub use connection::Connection;

/// Address of a LIFX device. This includes both the UDP socket address and the MAC address-based target filter.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct DeviceAddress {
    service_address: SocketAddr,
    target: DeviceTarget,
}

impl DeviceAddress {
    pub const fn new(service_address: SocketAddr, target: DeviceTarget) -> DeviceAddress {
        DeviceAddress {
            service_address,
            target,
        }
    }

    pub fn all() -> DeviceAddress {
        let udp_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(255, 255, 255, 255)), 56700);
        DeviceAddress::new(udp_address, DeviceTarget::All)
    }
}

impl fmt::Display for DeviceAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}@{}", self.target, self.service_address)
    }
}
