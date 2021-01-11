use std::net::SocketAddr;

use bytes::{Buf, BytesMut};
use futures::stream::FuturesUnordered;
use tokio::net::UdpSocket;
use tokio::sync::broadcast;

use lifx_client::{Client, Connection, DeviceAddress, AnyMessage};
use lifx_proto::device;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_ansi(true)
        .with_max_level(tracing::Level::TRACE)
        .init();

    let (mut client, conn) = Client::connect(1234).await.unwrap();
    tokio::spawn(async {
        if let Err(err) = conn.await {
            tracing::error!("Connection died: {}", err);
        }
    });

    let mut discovery = client.send_discovery().unwrap();
    loop {
        let address = discovery.recv().await.unwrap();
        tracing::info!("Discovered {}", address);

        let res = client.send_with_response(address, AnyMessage::GetLabel(device::GetLabel {})).await.unwrap();
        match res.message() {
            AnyMessage::StateLabel(inner) => {
                tracing::info!("Label for {} is {}", address, inner.label)
            },
            other => tracing::error!("Unexpected response to GetLabel! {:?}", other)
        }
    }
}
