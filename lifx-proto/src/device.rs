//! Device Messages

use bytes::{Buf, BufMut};

use crate::{Message, LifxError, wire, label::Label};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct GetService {}

/// `StateService` message, sent by devices as a response to [`GetService`] during discovery
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct StateService {
    pub service: Service,
    pub port: u32,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct GetLabel {}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct StateLabel {
    pub label: Label
}

/// Service exposed by a LIFX device
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Service {
    Udp,
    Unknown(u8)
}

impl Message for GetService {
    const TYPE: wire::MessageType = wire::MessageType::GetService;

    const PAYLOAD_SIZE: usize = 0;

    fn write_payload<B: BufMut>(&self, buf: &mut B) {
        // nothing to do
    }

    fn from_wire<B: Buf>(_header: &wire::MessageHeader, _buf: &mut B) -> Result<Self, LifxError> {
        Ok(GetService {})
    }
}

impl Message for StateService {
    const TYPE: wire::MessageType = wire::MessageType::StateService;

    const PAYLOAD_SIZE: usize = 5; // u8 for service and u32 for port

    fn write_payload<B: BufMut>(&self, buf: &mut B) {
        buf.put_u8(self.service.into());
        buf.put_u32_le(self.port);
    }

    fn from_wire<B: Buf>(_header: &wire::MessageHeader, buf: &mut B) -> Result<Self, LifxError> {
        let service = Service::from(buf.get_u8());
        let port = buf.get_u32_le();
        Ok(StateService { service, port })
    }
}

impl Message for GetLabel {
    const TYPE: wire::MessageType = wire::MessageType::GetLabel;

    const PAYLOAD_SIZE: usize = 0;

    fn write_payload<B: BufMut>(&self, _buf: &mut B) {
        
    }

    fn from_wire<B: Buf>(_header: &wire::MessageHeader, _buf: &mut B) -> Result<Self, LifxError> {
        Ok(GetLabel {})
    }
}

impl Message for StateLabel {
    const TYPE: wire::MessageType = wire::MessageType::StateLabel;

    const PAYLOAD_SIZE: usize = Label::MAX_LENGTH;

    fn write_payload<B: BufMut>(&self, buf: &mut B) {
        self.label.encode(buf).expect("write_payload called with insufficient buffer");
    }

    fn from_wire<B: Buf>(_header: &wire::MessageHeader, buf: &mut B) -> Result<Self, LifxError> {
        Ok(StateLabel {
            label: Label::decode(buf)?
        })
    }
}

impl Into<u8> for Service {
    fn into(self) -> u8 {
        match self {
            Service::Udp => 1,
            Service::Unknown(id) => id,
        }
    }
}

impl From<u8> for Service {
    fn from(value: u8) -> Service {
        match value {
            1 => Service::Udp,
            _ => Service::Unknown(value)
        }
    }
}