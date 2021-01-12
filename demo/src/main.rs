use std::time::Duration;

use lifx_client::Client;
use lifx_proto::color::{Hsbk, Kelvin};
use palette::Srgb;

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
        let state = client.get_light_state(address).await.unwrap();
        tracing::info!("State of {}: {:?}", address, state);
        let old_color: Srgb = state.color.color().into();
        tracing::info!("Original color: {:?}", old_color);

        let new_color = Hsbk::new(Srgb::new(0u8, 255, 255).into_format().into(), Kelvin::new(2700));
        client.set_light_color(address, new_color, Duration::from_secs(5)).await.unwrap();
        tracing::info!("Set color!");
        break;
    }
}
