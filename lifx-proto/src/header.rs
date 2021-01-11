//! On-the-wire representations of the LIFX LAN protocol

use std::convert::TryFrom;
use std::fmt;

use bit_field::BitField;
use bytes::{Buf, BufMut};
use macaddr::MacAddr6;

use crate::{ProtocolError, message::MessageType};

/// Header for LIFX messages.
///
/// The LIFX documentation splits the header into three sections, the Frame, Frame Address, and Protocol Header. Since all three sections are required, and they
/// reference each other, they are combined into one `Header` struct here.
///
/// See the [header description documentation](https://lan.developer.lifx.com/docs/header-description).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    /// Size of the entire message in bytes.
    pub size: u16,
    /// Source identifier. Clients can set this to a non-zero value, in which case devices will send responses to
    /// only the client that sent the original message. Otherwise, devices will broadcast their response to all clients.
    pub source: u32,

    /// Device being targeted. The message may be targeted at all devices when performing discovery. When responding to discovery, devices will
    /// set their own MAC address as the target address.
    pub target: DeviceTarget,

    /// Whether or not a response message is required
    pub response_required: bool,
    /// Whether or not an acknowledgement message is required
    pub acknowledgement_required: bool,

    /// A sequence number, which may wrap around. Devices will include the sequence number in any response messages they send.
    pub sequence: u8,

    /// The type of message this is
    pub message_type: MessageType,
}

/// Address of the device(s) a message is being sent to/from.
///
/// This corresponds to the `tagged` field of the Frame section and the `target` field of the Frame Address section.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DeviceTarget {
    All,
    Targeted(MacAddr6),
}


impl Header {
    /// Size of a LIFX message header
    /// * 8 bytes for the Frame section
    /// * 16 bytes for the Frame Address section
    /// * 12 bytes for the Protocol Header section
    pub const HEADER_SIZE: usize = 36;

    /// Expected protocol number
    const PROTOCOL_NUMBER: u16 = 1024;

    /// Deserialize a message header from an input buffer.
    pub fn decode<B: Buf>(buf: &mut B) -> Result<Header, ProtocolError> {
        // Parse the Frame header

        let size = buf.get_u16_le();

        // The protocol, addressable, tagged, and origin fields are all effectively in one u16
        let proto_flags = buf.get_u16_le();
        let protocol = proto_flags & 0x0FFF; // Get the lower 12 bits
        if protocol != Header::PROTOCOL_NUMBER {
            return Err(ProtocolError::InvalidProtocol(protocol));
        }
        let origin = proto_flags.get_bits(14..16);
        if origin != 0 {
            return Err(ProtocolError::InvalidOrigin(origin as u8));
        }
        let addressable = proto_flags.get_bit(12);
        if !addressable {
            return Err(ProtocolError::NotAddressable);
        }
        let tagged = proto_flags.get_bit(13);

        let source = buf.get_u32_le();

        // Parse the Frame Address

        let mut target = [0u8; 6];
        buf.copy_to_slice(&mut target);
        buf.advance(8); // Skip last 2 bytes after MAC address + 6 reserved bytes

        let address_flags = buf.get_u8();
        let response_required = address_flags.get_bit(0);
        let acknowledgement_required = address_flags.get_bit(1);

        let sequence = buf.get_u8();

        let target = if tagged {
            DeviceTarget::All
        } else {
            DeviceTarget::Targeted(target.into())
        };

        // Parse the Protocol Header
        buf.advance(8); // Skip 8 reserved bytes
        let message_type = MessageType::from(buf.get_u16_le());
        buf.advance(2); // Skip 2 reserved bytes

        Ok(Header {
            size,
            source,
            target,
            response_required,
            acknowledgement_required,
            sequence,
            message_type,
        })
    }

    /// Serialize a message header to an output buffer.
    pub fn encode<B: BufMut>(&self, buf: &mut B) {
        // Write the Frame header

        buf.put_u16_le(self.size);

        let mut proto_flags = Header::PROTOCOL_NUMBER;
        proto_flags.set_bit(12, true); // Set the addressable bit
        proto_flags.set_bit(13, matches!(self.target, DeviceTarget::All)); // Set the tagged bit
                                                                           // Origin will already be 0
        buf.put_u16_le(proto_flags);

        buf.put_u32_le(self.source);

        // Write the Frame Address

        // 8 bytes for the target
        match self.target {
            DeviceTarget::All => buf.put_u64(0),
            DeviceTarget::Targeted(target) => {
                buf.put_slice(target.as_bytes());
                buf.put_u16(0); // Last 2 bytes are 0
            }
        }

        buf.put_slice(&[0; 6]); // 6 reserved bytes

        let mut address_flags = 0u8;
        address_flags.set_bit(0, self.response_required);
        address_flags.set_bit(1, self.acknowledgement_required);
        buf.put_u8(address_flags);

        buf.put_u8(self.sequence);

        // Write the Protocol Header

        buf.put_u64(0); // 8 bytes of padding
        buf.put_u16_le(self.message_type.into());
        buf.put_u16(0); // 2 bytes of padding
    }

    /// The expected size of the payload following this header, in bytes
    pub fn payload_size(&self) -> usize {
        self.size as usize - Header::HEADER_SIZE
    }
}


impl fmt::Display for DeviceTarget {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DeviceTarget::All => f.write_str("all"),
            DeviceTarget::Targeted(addr) => addr.fmt(f),
        }
    }
}

#[test]
fn test_header() {
    let mut bytes: &[u8] = &[0b0000_0000, 0b0011_0100];
    let proto_flags = bytes.get_u16_le();

    // Protocol version
    assert_eq!(proto_flags & 0x0FFF, 1024);
    // Origin
    assert_eq!(proto_flags.get_bits(14..), 0);
    // Addressable
    assert_eq!(proto_flags.get_bit(12), true);
    // Tagged
    assert_eq!(proto_flags.get_bit(13), true);
}
