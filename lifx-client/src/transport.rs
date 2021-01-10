use std::collections::{HashMap, hash_map::Entry};
use std::fmt;
use std::net::{SocketAddr, IpAddr, Ipv4Addr};

use bytes::BytesMut;
use lifx_proto::{self, LifxError, PacketOptions, device::{Service, self}, wire::{DeviceTarget, MessageHeader, WireError}};
use tokio::net::UdpSocket;
use tokio::sync::{broadcast, oneshot};
use thiserror::Error;

use crate::any_message::AnyMessage;

const BUFFER_SIZE: usize = 512;

pub struct Transport {
    socket: UdpSocket,
    buffer: BytesMut,

    source: u32,
    sequence_number: u8,
    pending: HashMap<u8, PendingResponse>,
    /// Broadcast sender for discovered devices. Discovery messages (GetService and StateService) have extra support built in to [`Transport`]
    /// because they don't follow the standard 1:1 request/response model of other messages. Instead, we might get a StateService message any time after
    /// sending a GetService
    discovery: broadcast::Sender<DeviceAddress>,
}

/// Address of a LIFX device. This includes both the UDP socket address and the MAC address-based target filter.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct DeviceAddress {
    // TODO: maybe rename to service_address?
    udp_address: SocketAddr,
    target: DeviceTarget,
}

#[derive(Debug, Error)]
pub enum TransportError {
    /// We ran out of sequence numbers. This happens if there are too many outstanding messages that require a response or acknowledgement.
    #[error("ran out of sequence numbers")]
    SequenceExhausted,

    #[error("protocol error: {0}")]
    Protocol(#[from] LifxError),

    #[error("network error: {0}")]
    Network(#[from] std::io::Error)
}

pub enum PendingResponse {
    AckExpected(oneshot::Sender<()>),
    ResponseExpected(oneshot::Sender<AnyMessage>)
}

impl Transport {
    pub fn new(socket: UdpSocket, source: u32, discovery: broadcast::Sender<DeviceAddress>) -> Transport {
        Transport {
            socket,
            buffer: BytesMut::with_capacity(BUFFER_SIZE),
            source,
            sequence_number: 1,
            pending: HashMap::new(),
            discovery,
        }
    }

    #[inline]
    fn next_sequence(&mut self) -> u8 {
        let seq = self.sequence_number;
        self.sequence_number = self.sequence_number.wrapping_add(1);
        seq
    }

    pub async fn send_discovery(&mut self) ->Result<(), TransportError> {
        self.send_and_forget(&AnyMessage::GetService(device::GetService { }), DeviceAddress::all()).await?;
        Ok(())
    }

    pub async fn send_and_forget(&mut self, message: &AnyMessage, address: DeviceAddress) -> Result<(), TransportError> {
        let sequence = self.next_sequence();
        let options = PacketOptions {
            source: self.source,
            sequence,
            target: address.target,
            acknowledgement_required: false,
            response_required: false,
        };
        self.send_message(message, address, &options).await?;

        Ok(())
    }

    pub async fn send_with_response(&mut self, message: &AnyMessage, address: DeviceAddress, response: PendingResponse) -> Result<(), TransportError> {
        let (response_required, ack_required) = match response {
            PendingResponse::ResponseExpected(_) => (true, false),
            PendingResponse::AckExpected(_) => (false, true)
        };

        // Before sending anything, make sure we haven't run out of sequence numbers for pending responses
        let sequence = self.next_sequence();
        match self.pending.entry(sequence) {
            Entry::Vacant(entry) => entry.insert(response),
            Entry::Occupied(_) => return Err(TransportError::SequenceExhausted)
        };

        let options = PacketOptions {
            source: self.source,
            sequence,
            target: address.target,
            acknowledgement_required: ack_required,
            response_required,
        };
        self.send_message(message, address, &options).await?;

        Ok(())
    }

    async fn send_message(&mut self, message: &AnyMessage, address: DeviceAddress, options: &PacketOptions) -> Result<(), TransportError> {
        self.buffer.resize(message.packet_size(), 0);
        message.encode(options, &mut self.buffer)?;
        self.socket.send_to(&self.buffer[..], address.udp_address).await?;
        Ok(())
    }

    /// Waits for this [`Transport`] to have pending messages.
    pub async fn has_messages(&self) -> Result<(), TransportError> {
        self.socket.readable().await?;
        Ok(())
    }

    /// Process any pending incoming messages
    pub fn process_messages(&mut self) -> Result<(), TransportError> {
        // Implementation note: this is a non-async, non-blocking method so that higher-level code can send and receive at the same time
        loop {
            self.buffer.resize(BUFFER_SIZE, 0);
            match self.socket.try_recv_from(&mut self.buffer) {
                Ok((len, addr)) => {
                    tracing::trace!("Received {} bytes from {}", len, addr);
                    self.buffer.truncate(len);
                    use bytes::Buf;
                    assert_eq!(self.buffer.get_u16_le() as usize, len);
                    let header = MessageHeader::parse(&mut self.buffer)?;
                    tracing::debug!("Incoming message from {}: {:?}", addr, header);
                    if header.source != self.source {
                        // The message wasn't for us, skip it
                        continue;
                    }

                    let message = AnyMessage::decode(&mut self.buffer, &header)?;
                    match message {
                        AnyMessage::StateService(service) => {
                            let address = DeviceAddress::new(SocketAddr::new(addr.ip(), service.port as u16), header.target);
                            match service.service {
                                Service::Udp => {
                                    // TODO: verify that target is not All
                                    tracing::debug!("Discovered {}", address);
                                    if let Err(err) = self.discovery.send(address) {
                                        tracing::warn!("Sending discovery update failed: {}", err);
                                    }
                                },
                                other => {
                                    tracing::trace!("Received unknown service announcement for {:?} from {}", other, address);
                                }
                            }
                        },
                        _ => {
                            // TODO: acknowledgements
                            match self.pending.remove(&header.sequence) {
                                Some(PendingResponse::ResponseExpected(sender)) => {
                                    if let Err(_) = sender.send(message) {
                                        tracing::warn!("Dispatching response failed");
                                    }
                                }
                                _ => {
                                    tracing::debug!("Ignoring unexpected response with sequence number {}: {:?}", header.sequence, message);
                                }
                            };
                        }
                    }
                },
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
                Err(e) => return Err(e.into()),
            }
        }
    }
}

impl DeviceAddress {
    pub const fn new(udp_address: SocketAddr, target: DeviceTarget) -> DeviceAddress {
        DeviceAddress { udp_address, target }
    }

    pub fn all() -> DeviceAddress {
        let udp_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(255, 255, 255, 255)), 56700);
        DeviceAddress::new(udp_address, DeviceTarget::All)
    }
}

impl fmt::Display for DeviceAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}@{}", self.target, self.udp_address)
    }
}

impl From<WireError> for TransportError {
    fn from(err: WireError) -> TransportError {
        TransportError::Protocol(LifxError::Wire(err))
    }
}