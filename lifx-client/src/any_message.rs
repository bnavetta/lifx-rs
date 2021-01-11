use bytes::{Buf, BufMut};
use lifx_proto::{
    self, device,
    wire::{MessageHeader, MessageType},
    LifxError, Message, PacketOptions,
};

/// Wrapper for [`Message`] types. This allows passing around a generic message, which is useful in certain cases (such as dispatching responses).
/// In general, however, it's preferable to deal with a concrete, specific [`Message`] type.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum AnyMessage {
    GetService(device::GetService),
    StateService(device::StateService),
    GetLabel(device::GetLabel),
    StateLabel(device::StateLabel),
}

impl AnyMessage {
    pub fn decode<B: Buf>(buf: &mut B, header: &MessageHeader) -> Result<AnyMessage, LifxError> {
        match header.message_type {
            MessageType::GetService => Ok(AnyMessage::GetService(lifx_proto::decode_packet(
                header, buf,
            )?)),
            MessageType::StateService => Ok(AnyMessage::StateService(lifx_proto::decode_packet(
                header, buf,
            )?)),
            MessageType::GetLabel => Ok(AnyMessage::GetLabel(lifx_proto::decode_packet(
                header, buf,
            )?)),
            MessageType::StateLabel => Ok(AnyMessage::StateLabel(lifx_proto::decode_packet(
                header, buf,
            )?)),
            _ => Err(LifxError::UnexpectedMessage {
                message_type: header.message_type,
            }),
        }
    }

    pub fn encode<B: BufMut>(&self, options: &PacketOptions, buf: &mut B) -> Result<(), LifxError> {
        match self {
            AnyMessage::GetService(inner) => lifx_proto::encode_packet(options, inner, buf),
            AnyMessage::StateService(inner) => lifx_proto::encode_packet(options, inner, buf),
            AnyMessage::GetLabel(inner) => lifx_proto::encode_packet(options, inner, buf),
            AnyMessage::StateLabel(inner) => lifx_proto::encode_packet(options, inner, buf),
        }
    }

    pub fn packet_size(&self) -> usize {
        let payload_size = match self {
            AnyMessage::GetService(_) => device::GetService::PAYLOAD_SIZE,
            AnyMessage::StateService(_) => device::StateService::PAYLOAD_SIZE,
            AnyMessage::GetLabel(_) => device::GetLabel::PAYLOAD_SIZE,
            AnyMessage::StateLabel(_) => device::StateLabel::PAYLOAD_SIZE,
        };
        payload_size + MessageHeader::HEADER_SIZE
    }
}
