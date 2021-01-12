//! Representation of the LIFX LAN protocol

use std::convert::TryInto;

use bytes::{Buf, BufMut};
use thiserror::Error;

pub mod color;
pub mod label;
pub mod message;
pub mod header;

pub use message::{Message, MessageType, Service};
pub use header::{DeviceTarget, Header};

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("unexpected message: {0:?}")]
    UnexpectedMessage(MessageType),

    #[error("invalid protocol number: {0}")]
    InvalidProtocol(u16),

    #[error("message not marked as addressable")]
    NotAddressable,

    #[error("invalid origin indicator: {0}")]
    InvalidOrigin(u8),

    #[error("invalid label")]
    InvalidLabel,

    #[error("invalid payload: {0}")]
    InvalidPayload(String),
}

#[derive(Debug, Clone)]
pub struct Packet {
    source: u32,
    target: DeviceTarget,
    sequence: u8,
    response_required: bool,
    acknowledgement_required: bool,
    message: Message,
}

impl Packet {
    pub fn new(source: u32, target: DeviceTarget, sequence: u8, response_required: bool, acknowledgement_required: bool, message: Message) -> Packet {
        Packet {
            source,
            target,
            sequence,
            response_required,
            acknowledgement_required,
            message
        }
    }

    pub fn encode<B: BufMut>(&self, buf: &mut B) {
        let size = self.len().try_into().expect("Packet size larger than u16");
        let header = Header {
            size,
            source: self.source,
            target: self.target,
            sequence: self.sequence,
            response_required: self.response_required,
            acknowledgement_required: self.acknowledgement_required,
            message_type: self.message.message_type(),
        };
        header.encode(buf);
        self.message.encode_payload(buf);
    }

    pub fn decode<B: Buf>(buf: &mut B) -> Result<Packet, ProtocolError> {
        let header = Header::decode(buf)?;
        let message = Message::decode(&header, buf)?;
        Ok(Packet {
            source: header.source,
            target: header.target,
            sequence: header.sequence,
            response_required: header.response_required,
            acknowledgement_required: header.acknowledgement_required,
            message
        })
    }

    pub fn len(&self) -> usize {
        Header::HEADER_SIZE + self.message.payload_size()
    }

    pub fn source(&self) -> u32 {
        self.source
    }

    pub fn sequence(&self) -> u8 {
        self.sequence
    }

    pub fn target(&self) -> DeviceTarget {
        self.target
    }

    pub fn message(&self) -> &Message {
        &self.message
    }

    pub fn into_message(self) -> Message {
        self.message
    }
}
