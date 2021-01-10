use std::net::SocketAddr;

use bytes::{Buf, BytesMut};
use tokio::net::UdpSocket;
use tokio::sync::broadcast;

use lifx_client::{DeviceAddress, transport::Transport};
use lifx_proto::device;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_ansi(true)
        .with_max_level(tracing::Level::TRACE)
        .init();

    let socket = UdpSocket::bind("0.0.0.0:56700").await.unwrap();
    socket.set_broadcast(true).unwrap();

    let (discovery_tx, mut discovery) = broadcast::channel(20);
    let mut transport = Transport::new(socket, 1234, discovery_tx);

    tracing::info!("Sending GetState message...");
    transport.send_discovery().await.unwrap();
    tracing::info!("Hi");

    loop {
        transport.process_messages().unwrap();

        if let Ok(addr) = discovery.try_recv() {
            tracing::info!("Discovered {}", addr);
        }
    }

    // TODO: get labels
}
