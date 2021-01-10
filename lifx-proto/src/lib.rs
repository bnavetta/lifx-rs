//! Representation of the LIFX LAN protocol

use bytes::{Buf, BufMut};
use thiserror::Error;

pub mod device;
pub mod label;
pub mod wire;

#[derive(Debug, Error)]
pub enum LifxError {
    #[error("wire error: {0}")]
    Wire(#[from] wire::WireError),

    #[error("payload error: {0}")]
    Payload(#[from] PayloadError),

    #[error("unexpected message: {message_type:?}")]
    UnexpectedMessage {
        message_type: wire::MessageType
    },

    #[error("invalid label")]
    InvalidLabel
}

/// Errors related to a message payload
#[derive(Debug, Error)]
pub enum PayloadError {
}

/// A LIFX protocol message
pub trait Message: Sized {
    /// Type of this message in the wire protocol
    const TYPE: wire::MessageType;

    /// Size in bytes of this message's payload
    const PAYLOAD_SIZE: usize;

    /// Write the message payload into the given buffer.
    /// Implementors may assume that the buffer has at least [`Self::payload_size`] bytes remaining.
    fn write_payload<B: BufMut>(&self, buf: &mut B);

    /// Parses the message from a packet header and buffer for the payload.
    /// Implementors may assume that the header's [`wire::MessageHeader::message_type`] matches [`Self::TYPE`] and that the buffer has at least
    /// [`Self::payload_size`] bytes remaining.
    fn from_wire<B: Buf>(header: &wire::MessageHeader, buf: &mut B) -> Result<Self, LifxError>;
}

/// Packet information common to all message types
pub struct PacketOptions {
    pub source: u32,
    pub target: wire::DeviceTarget,
    pub sequence: u8,
    pub response_required: bool,
    pub acknowledgement_required: bool,
}

/// Encode a LIFX packet into `buf`. This writes both the message header and the payload.
///
/// # Arguments
/// * `options` - packet options such as the target device
/// * `message` - the message to encode
/// * `buf` - buffer to write into
pub fn encode_packet<M: Message, B: BufMut>(options: &PacketOptions, message: &M, buf: &mut B) -> Result<(), LifxError> {
    let size = wire::MessageHeader::HEADER_SIZE + M::PAYLOAD_SIZE;

    write_header(buf, size, M::TYPE, options)?;
    message.write_payload(buf);

    Ok(())
}

/// Generates and writes the packet header for a message.
/// This reduces the amount of specialized code generated for [`encode_packet`] calls.
fn write_header<B: BufMut>(buf: &mut B, size: usize, message_type: wire::MessageType, options: &PacketOptions) -> Result<(), LifxError> {
    debug_assert!(size < std::u16::MAX as usize, "Packet too large!");
    if size > buf.remaining_mut() {
        return Err(LifxError::Wire(wire::WireError::InsufficientData {
            available: buf.remaining_mut(),
            needed: size
        }));
    }


    let header = wire::MessageHeader {
        size: size as u16,
        source: options.source,
        target: options.target,
        response_required: options.response_required,
        acknowledgement_required: options.acknowledgement_required,
        sequence: options.sequence,
        message_type: message_type
    };

    header.write(buf)?;
    Ok(())
}

/// Read a LIFX packet from a buffer, after the message header has already been parsed.
///
/// # Arguments
/// * `header` - the message header
/// * `buf` - buffer to read the message payload from
pub fn decode_packet<M: Message, B: Buf>(header: &wire::MessageHeader, buf: &mut B) -> Result<M, LifxError> {
    verify_header(buf, header, M::TYPE, M::PAYLOAD_SIZE)?;
    M::from_wire(header, buf)
}

/// Verifies that a message header matches the expected type and size
/// This reduces the amout of specialized code generated for [`decode_packet`] calls
fn verify_header<B: Buf>(buf: &B, header: &wire::MessageHeader, expected_type: wire::MessageType, expected_size: usize) -> Result<(), LifxError> {
    if header.message_type != expected_type {
        Err(LifxError::UnexpectedMessage {
            message_type: header.message_type,
        })
    } else if buf.remaining() < expected_size {
        Err(LifxError::Wire(wire::WireError::InsufficientData {
            available: buf.remaining(),
            needed: expected_size
        }))
    } else {
        Ok(())
    }
}