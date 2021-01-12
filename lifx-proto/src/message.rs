use std::time::Duration;

use bytes::{BufMut, Buf};

use crate::ProtocolError;
use crate::color::Hsbk;
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

    Acknowledgement,

    // Light messages
    Get,
    SetColor(SetColor),
    State(State),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum MessageType {
    GetService,
    StateService,

    GetLabel,
    SetLabel,
    StateLabel,

    Acknowledgement,

    Get,
    SetColor,
    State,

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetColor {
    pub color: Hsbk,
    /// Color transition time
    pub duration: Duration,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct State {
    pub color: Hsbk,
    pub power: u16,
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
            Message::Acknowledgement => MessageType::Acknowledgement,
            Message::Get => MessageType::Get,
            Message::SetColor(_) => MessageType::SetColor,
            Message::State(_) => MessageType::State,

        }
    }

    pub fn payload_size(&self) -> usize {
        match self {
            Message::GetService => 0,
            Message::StateService(_) => 5,
            Message::GetLabel => 0,
            Message::SetLabel(_) => Label::MAX_LENGTH,
            Message::StateLabel(_) => Label::MAX_LENGTH,
            Message::Acknowledgement => 0,
            Message::Get => 0,
            Message::SetColor(_) => 1 /* reserved */ + Hsbk::SIZE + 4 /* duration */,
            Message::State(_) =>  Hsbk::SIZE + 2 /* reserved */ + 2 /* power */ + Label::MAX_LENGTH + 8 /* reserved */,
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
            },
            Message::Acknowledgement => (),
            Message::Get => (),
            Message::SetColor(inner) => {
                buf.put_u8(0); // reserved
                inner.color.encode(buf);
                buf.put_u32_le(inner.duration.as_millis() as u32);
            },
            Message::State(inner) => {
                inner.color.encode(buf);
                buf.put_i16_le(0); // reserved
                buf.put_u16_le(inner.power);
                inner.label.encode(buf);
                buf.put_u64_le(0); // reserved
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
            },
            MessageType::Acknowledgement => Ok(Message::Acknowledgement),
            MessageType::Get => Ok(Message::Get),
            MessageType::SetColor => {
                let _ = buf.get_u8(); // reserved
                let color = Hsbk::decode(buf)?;
                let duration = Duration::from_millis(buf.get_u32_le().into());
                Ok(Message::SetColor(SetColor { color, duration }))
            }
            MessageType::State => {
                let color = Hsbk::decode(buf)?;
                let _ = buf.get_i16_le(); // reserved
                let power = buf.get_u16_le();
                let label = Label::decode(buf)?;
                let _ = buf.get_u64_le(); // reserved
                Ok(Message::State(State { color, power, label }))
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
            45 => MessageType::Acknowledgement,
            101 => MessageType::Get,
            102 => MessageType::SetColor,
            107 => MessageType::State,
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
            MessageType::Acknowledgement => 45,
            MessageType::Get => 101,
            MessageType::SetColor => 102,
            MessageType::State => 107,
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
