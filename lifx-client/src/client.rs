use std::time::Duration;

use lifx_proto::{Message, ProtocolError, message::*, color::Hsbk};
use tokio::net::{UdpSocket, ToSocketAddrs};
use tokio::sync::{mpsc, broadcast, oneshot};
use tokio_util::udp::UdpFramed;

use crate::DeviceAddress;
use crate::codec::Codec;
use crate::connection::{Connection, Request, Response, InboundMessage};
use crate::error::Error;

pub struct Client {
    requests: mpsc::UnboundedSender<Request>,
    // Only needed for Clone
    discovery_tx: broadcast::Sender<DeviceAddress>,
    discovery: broadcast::Receiver<DeviceAddress>,
}

impl Client {
    /// Create a new `Client` bound to the LIFX-recommended address (`0.0.0.0:56700`). This port is recommended because older LIFX devices send replies to port 56700 instead
    /// of checking the port that the original message came from.
    pub async fn connect(source: u32) -> Result<(Client, Connection), Error> {
        Client::with_address_and_source("0.0.0.0:56700", source).await
    }

    /// Create a new `Client` bound to `addr`
    pub async fn with_address_and_source<A: ToSocketAddrs>(addr: A, source: u32) -> Result<(Client, Connection), Error> {
        let socket = UdpSocket::bind(addr).await?;
        Client::with_socket_and_source(socket, source)
    }

    pub fn with_socket_and_source(socket: UdpSocket, source: u32) -> Result<(Client, Connection), Error> {
        socket.set_broadcast(true)?; // Needed for discovery

        let (request_tx, request_rx) = mpsc::unbounded_channel();
        let (discovery_tx, discovery_rx) = broadcast::channel(10);
        let conn = Connection::new(
            UdpFramed::new(socket, Codec),
            source,
            request_rx,
            discovery_tx.clone()
        );

        let client = Client {
            requests: request_tx,
            discovery: discovery_rx,
            discovery_tx,
        };

        Ok((client, conn))
    }

    // Higher-level operations

    pub fn send_discovery(&mut self) -> Result<broadcast::Receiver<DeviceAddress>, Error> {
        self.send_async(DeviceAddress::all(), Message::GetService)?;
        Ok(self.discovery_tx.subscribe())
    }
    
    pub async fn get_label(&mut self, address: DeviceAddress) -> Result<String, Error> {
        let message = self.send_with_response(address, Message::GetLabel).await?;
        match message.into_message() {
            Message::StateLabel(inner) => Ok(inner.label.into_string()),
            other => Err(Error::Protocol(ProtocolError::UnexpectedMessage(other.message_type())))
        }
    }

    pub async fn get_light_state(&mut self, address: DeviceAddress) -> Result<State, Error> {
        let message = self.send_with_response(address, Message::Get).await?;
        match message.into_message() {
            Message::State(inner) => Ok(inner),
            other => Err(Error::Protocol(ProtocolError::UnexpectedMessage(other.message_type())))
        }
    }

    pub async fn set_light_color(&mut self, address: DeviceAddress, color: Hsbk, transition_duration: Duration) -> Result<(), Error> {
        let message = Message::SetColor(SetColor { color, duration: transition_duration });
        // TODO: flag for sending async or not
        self.send_with_acknowledgement(address, message).await
    }

    // Lower-level functions to send/receive messages directly

    pub fn send_async(&mut self, address: DeviceAddress, message: Message) -> Result<(), Error> {
        self.send(Request::new(address, message, None))
    }

    pub async fn send_with_response(&mut self, address: DeviceAddress, message: Message) -> Result<InboundMessage, Error> {
        let (tx, rx) = oneshot::channel();
        self.send(Request::new(address, message, Some(Response::Reply(tx))))?;
        rx.await.map_err(|_| Error::ConnectionClosed)
    }

    pub async fn send_with_acknowledgement(&mut self, address: DeviceAddress, message: Message) -> Result<(), Error> {
        let (tx, rx) = oneshot::channel();
        self.send(Request::new(address, message, Some(Response::Acknowledgement(tx))))?;
        rx.await.map_err(|_| Error::ConnectionClosed)
    }

    fn send(&mut self, request: Request) -> Result<(), Error> {
        self.requests.send(request).map_err(|_| Error::ConnectionClosed)
    }
}

impl Clone for Client {
    fn clone(&self) -> Self {
        let discovery_tx = self.discovery_tx.clone();
        Client {
            requests: self.requests.clone(),
            discovery: discovery_tx.subscribe(),
            discovery_tx,
        }
    }
}
// TODO: better discovery handling?