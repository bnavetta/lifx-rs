use bytes::{Buf, BytesMut};
use lifx_proto::{wire::MessageHeader, PacketOptions};
use tokio_util::codec::{Decoder, Encoder};

use crate::any_message::AnyMessage;
use crate::error::Error;

pub struct Codec;

/// Maximum allowed packet size. Packets larger than this will be rejected, to prevent potential denial-of-service attacks.
const MAX_PACKET_SIZE: usize = 4 * 1024;

impl Encoder<(PacketOptions, AnyMessage)> for Codec {
    type Error = Error;

    fn encode(
        &mut self,
        (options, message): (PacketOptions, AnyMessage),
        dst: &mut BytesMut,
    ) -> Result<(), Self::Error> {
        dst.reserve(message.packet_size());
        message.encode(&options, dst)?;
        Ok(())
    }
}

impl Decoder for Codec {
    type Item = (MessageHeader, AnyMessage);

    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // TODO: since we're dealing with UDP, should we reject if there's insufficient data instead of returning Ok(None)?
        if src.len() < MessageHeader::HEADER_SIZE {
            // Not enough data to read the message header
            src.reserve(MessageHeader::HEADER_SIZE);
            return Ok(None);
        }

        // Peek at the message size field so we know if all the data has arrived
        let mut size_bytes = [0u8; 2];
        size_bytes.copy_from_slice(&src[..2]);
        let size = u16::from_le_bytes(size_bytes) as usize;

        if size > MAX_PACKET_SIZE {
            return Err(Error::Network(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Packet of length {} is too large", size),
            )));
        }

        if src.len() < size {
            // The full packet has not arrived, so reserve capacity for when it does and signal that more data is needed
            src.reserve(size);
            return Ok(None);
        }

        let mut data = &src[..size];
        let header = MessageHeader::parse(&mut data)?;
        let message = AnyMessage::decode(&mut data, &header)?;
        src.advance(size);

        Ok(Some((header, message)))
    }
}
