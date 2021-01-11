use bytes::{BufMut, Buf};

use crate::ProtocolError;
use crate::header::Header;
use crate::label::Label;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    // Device messages
    GetService,
    StateService(StateService),
    GetLabel,
    SetLabel(SetLabel),
    StateLabel(StateLabel),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum MessageType {
    GetService,
    StateService,

    GetLabel,
    SetLabel,
    StateLabel,

    Other(u16),
}


/// Payload of a `StateService` [`Message`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateService {
    pub service: Service,
    pub port: u32,
}

/// Payload of a `SetLabel` [`Message`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetLabel {
    pub label: Label,
}

/// Payload of a `StateLabel` [`Message`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateLabel {
    pub label: Label,
}

/// Service exposed by a LIFX device
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Service {
    Udp,
    Unknown(u8),
}


impl Message {
    pub fn message_type(&self) -> MessageType {
        match self {
            Message::GetService => MessageType::GetService,
            Message::StateService(_) => MessageType::StateService,
            Message::GetLabel => MessageType::GetLabel,
            Message::SetLabel(_) => MessageType::SetLabel,
            Message::StateLabel(_) => MessageType::StateLabel,

        }
    }

    pub fn payload_size(&self) -> usize {
        match self {
            Message::GetService => 0,
            Message::StateService(_) => 5,
            Message::GetLabel => 0,
            Message::SetLabel(_) => Label::MAX_LENGTH,
            Message::StateLabel(_) => Label::MAX_LENGTH,
        }
    }

    pub(crate) fn encode_payload<B: BufMut>(&self, buf: &mut B) {
        match self {
            Message::GetService => (),
            Message::StateService(service) => {
                buf.put_u8(service.service.into());
                buf.put_u32_le(service.port);
            },
            Message::GetLabel => (),
            Message::SetLabel(inner) => {
                inner.label.encode(buf);
            },
            Message::StateLabel(inner) => {
                inner.label.encode(buf);
            }
        }
    }

    pub(crate) fn decode<B: Buf>(header: &Header, buf: &mut B) -> Result<Message, ProtocolError> {
        match header.message_type {
            MessageType::GetService => Ok(Message::GetService),
            MessageType::StateService => {
                let service = Service::from(buf.get_u8());
                let port = buf.get_u32_le();
                Ok(Message::StateService(StateService { service, port }))
            },
            MessageType::GetLabel => Ok(Message::GetLabel),
            MessageType::SetLabel => {
                let label = Label::decode(buf)?;
                Ok(Message::SetLabel(SetLabel { label }))
            }
            MessageType::StateLabel => {
                let label = Label::decode(buf)?;
                Ok(Message::StateLabel(StateLabel { label }))
            }
            MessageType::Other(_) => Err(ProtocolError::UnexpectedMessage(header.message_type))
        }
    }
}

impl From<u16> for MessageType {
    fn from(value: u16) -> MessageType {
        match value {
            2 => MessageType::GetService,
            3 => MessageType::StateService,
            23 => MessageType::GetLabel,
            24 => MessageType::SetLabel,
            25 => MessageType::StateLabel,
            _ => MessageType::Other(value),
        }
    }
}

impl Into<u16> for MessageType {
    fn into(self) -> u16 {
        match self {
            MessageType::GetService => 2,
            MessageType::StateService => 3,
            MessageType::GetLabel => 23,
            MessageType::SetLabel => 24,
            MessageType::StateLabel => 25,
            MessageType::Other(value) => value,
        }
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
            _ => Service::Unknown(value),
        }
    }
}
