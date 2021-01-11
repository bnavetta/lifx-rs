use lifx_proto::device;
use tokio::net::{UdpSocket, ToSocketAddrs};
use tokio::sync::{mpsc, broadcast, oneshot};
use tokio_util::udp::UdpFramed;

use crate::{AnyMessage, DeviceAddress};
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

    pub fn send_async(&mut self, address: DeviceAddress, message: AnyMessage) -> Result<(), Error> {
        self.send(Request::new(address, message, None))
    }

    pub async fn send_with_response(&mut self, address: DeviceAddress, message: AnyMessage) -> Result<InboundMessage, Error> {
        let (tx, rx) = oneshot::channel();
        self.send(Request::new(address, message, Some(Response::Reply(tx))))?;
        rx.await.map_err(|_| Error::ConnectionClosed)
    }

    pub fn send_discovery(&mut self) -> Result<broadcast::Receiver<DeviceAddress>, Error> {
        self.send_async(DeviceAddress::all(), AnyMessage::GetService(device::GetService {}))?;
        Ok(self.discovery_tx.subscribe())
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