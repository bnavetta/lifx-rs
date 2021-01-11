use std::collections::HashMap;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::{Sink, Stream};
use lifx_proto::{Packet, Message, Service};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio_util::udp::UdpFramed;

use crate::codec::Codec;
use crate::error::Error;
use crate::DeviceAddress;

// This is modeled on how tokio-postgres handles client I/O. The benefit of structuring things this way is that we can send messages and read responses
// simultaneously while presenting a convenient Futures-based API to callers
// See https://github.com/sfackler/rust-postgres/blob/77aa702e6c9052cddb256b56c5a8ad30f5272c0a/tokio-postgres/src/connection.rs

pub struct Request {
    address: DeviceAddress,
    message: Message,
    response: Option<Response>,
}

#[derive(Debug)]
pub struct InboundMessage {
    addr: SocketAddr,
    packet: Packet,
}
/// Expected response for a message
pub enum Response {
    Acknowledgement(oneshot::Sender<()>),
    Reply(oneshot::Sender<InboundMessage>),
}

/// Connection to LIFX devices on the local network.
///
/// This is the "backend" half of a LIFX client, which performs network I/O and handles protocol details. It should generally be executed in the background.
pub struct Connection {
    socket: UdpFramed<Codec>,
    source: u32,

    requests: mpsc::UnboundedReceiver<Request>,
    pending_request: Option<Request>,

    sequence_number: u8,

    pending_responses: HashMap<u8, Response>,

    discovery: broadcast::Sender<DeviceAddress>,
}

impl Connection {
    pub(crate) fn new(
        socket: UdpFramed<Codec>,
        source: u32,
        requests: mpsc::UnboundedReceiver<Request>,
        discovery: broadcast::Sender<DeviceAddress>,
    ) -> Connection {
        Connection {
            socket,
            source,
            requests,
            pending_request: None,
            sequence_number: 0,
            pending_responses: HashMap::new(),
            discovery,
        }
    }

    /// Polls for the next request to send out
    fn poll_request(&mut self, cx: &mut Context<'_>) -> Poll<Option<Request>> {
        // First, check if there's a pending message we had tried to send. This can happen if there's a false positive for the socket being writable
        if let Some(pending) = self.pending_request.take() {
            return Poll::Ready(Some(pending));
        }

        self.requests.poll_recv(cx)
    }

    /// Polls for incoming packets
    fn poll_incoming(&mut self, cx: &mut Context<'_>) -> Poll<Result<InboundMessage, Error>> {
        match Pin::new(&mut self.socket).poll_next(cx) {
            Poll::Ready(None) => Poll::Ready(Err(Error::Network(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "socket disconnected".to_string(),
            )))),
            Poll::Ready(Some(res)) => Poll::Ready(res.map(InboundMessage::from)),
            Poll::Pending => Poll::Pending,
        }
    }

    /// Dispatches a received message
    fn handle_message(&mut self, message: InboundMessage) {
        if message.packet.source() != self.source {
            tracing::trace!(
                "Skipping message for other source {}",
                message.packet.source()
            );
            return;
        }

        match self.pending_responses.remove(&message.packet.sequence()) {
            Some(response) => match response {
                Response::Reply(sender) => {
                    if let Err(m) = sender.send(message) {
                        tracing::warn!("Dangling response {:?}", m)
                    }
                }
                Response::Acknowledgement(_) => todo!("Handle acknowledgements"),
            },
            None => {
                if let Message::StateService(service) = message.packet.message() {
                    let address = DeviceAddress::new(
                        SocketAddr::new(message.addr.ip(), service.port as u16),
                        message.packet.target(),
                    );
                    match service.service {
                        Service::Udp => {
                            tracing::debug!("Discovered {}", address);
                            if let Err(_) = self.discovery.send(address) {
                                // TODO: shutdown here?
                                tracing::warn!("Discovery channel closed");
                            }
                        }
                        other => {
                            tracing::debug!(
                                "Encountered unknown service {:?} at {}",
                                other,
                                address
                            );
                        }
                    }
                } else {
                    tracing::trace!("Unexpected reply {:?}", message);
                }
            }
        }
    }

    /// Poll to send outgoing messages. This will send as many messages as possible, and returns `Ok(true)` if data was written to the socket and it needs to be flushed.
    fn poll_outgoing(&mut self, cx: &mut Context<'_>) -> Result<bool, Error> {
        loop {
            // First, check if the socket is writable
            if let Poll::Pending = Pin::new(&mut self.socket)
                .poll_ready(cx)
                .map_err(Error::from)?
            {
                return Ok(true);
            }

            let request = match self.poll_request(cx) {
                Poll::Ready(Some(request)) => request,
                Poll::Ready(None) => {
                    // TODO: shutdown
                    return Ok(true);
                }
                Poll::Pending => return Ok(true),
            };

            // We can only send if the sequence number is available. If too many messages are in flight, we'll have to wait for one to complete.
            match self.next_sequence(request.response.is_some()) {
                Some(sequence) => {
                    let (response_required, acknowledgement_required) = match request.response {
                        Some(res) => {
                            let flags = match res {
                                Response::Acknowledgement(_) => (false, true),
                                Response::Reply(_) => (true, false),
                            };
                            assert!(
                                self.pending_responses.insert(sequence, res).is_none(),
                                "next_sequence returned an in-use sequence number"
                            );
                            flags
                        }
                        None => (false, false),
                    };

                    let packet = Packet::new(self.source, request.address.target, sequence, response_required, acknowledgement_required, request.message);
                    Pin::new(&mut self.socket)
                        .start_send((packet, request.address.service_address))
                        .map_err(Error::from)?;
                }
                None => {
                    tracing::trace!("Deferring request, too many in flight");
                    self.pending_request = Some(request);
                    return Ok(true);
                }
            }
        }
    }

    fn poll_flush(&mut self, cx: &mut Context<'_>) -> Result<(), Error> {
        let _ = Pin::new(&mut self.socket)
            .poll_flush(cx)
            .map_err(Error::from)?;
        Ok(())
    }

    fn next_sequence(&mut self, has_response: bool) -> Option<u8> {
        if has_response {
            // Search through the sequence number space looking for one that doesn't correspond to a pending message
            // TODO: this could probably wrap around too
            for seq in self.sequence_number..=std::u8::MAX {
                if !self.pending_responses.contains_key(&seq) {
                    self.sequence_number = seq.wrapping_add(1);
                    return Some(seq);
                }
            }

            None
        } else {
            let seq = self.sequence_number;
            self.sequence_number = self.sequence_number.wrapping_add(1);
            Some(seq)
        }
    }
}

impl Future for Connection {
    type Output = Result<(), Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        while let Poll::Ready(msg) = self.poll_incoming(cx)? {
            self.handle_message(msg);
        }

        if self.poll_outgoing(cx)? {
            self.poll_flush(cx)?;
        }

        Poll::Pending // TODO: shutdown
    }
}


impl Request {
    pub fn new(address: DeviceAddress, message: Message, response: Option<Response>) -> Request {
        Request {
            address,
            message,
            response
        }
    }
}

impl InboundMessage {
    fn new(packet: Packet, addr: SocketAddr) -> InboundMessage {
        InboundMessage {
            packet, addr
        }
    }

    pub fn message(&self) -> &Message {
        &self.packet.message()
    }

    pub fn packet(&self) -> &Packet {
        &self.packet
    }
}

impl From<(Packet, SocketAddr)> for InboundMessage {
    fn from((packet, addr): (Packet, SocketAddr)) -> InboundMessage {
        InboundMessage::new(packet, addr)
    }
}