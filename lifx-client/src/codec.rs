use bytes::{Buf, BytesMut};
use lifx_proto::{Packet, Header};
use tokio_util::codec::{Decoder, Encoder};

use crate::error::Error;

pub struct Codec;

/// Maximum allowed packet size. Packets larger than this will be rejected, to prevent potential denial-of-service attacks.
const MAX_PACKET_SIZE: usize = 4 * 1024;

impl Encoder<Packet> for Codec {
    type Error = Error;

    fn encode(
        &mut self,
        packet: Packet,
        dst: &mut BytesMut,
    ) -> Result<(), Self::Error> {
        dst.reserve(packet.len());
        packet.encode(dst);
        Ok(())
    }
}

impl Decoder for Codec {
    type Item = Packet;

    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // TODO: since we're dealing with UDP, should we reject if there's insufficient data instead of returning Ok(None)?
        if src.len() < Header::HEADER_SIZE {
            // Not enough data to read the message header
            src.reserve(Header::HEADER_SIZE);
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
        let packet = Packet::decode(&mut data)?;
        src.advance(size);

        Ok(Some(packet))
    }
}
