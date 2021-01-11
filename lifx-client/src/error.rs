use lifx_proto::ProtocolError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("protocol error: {0}")]
    Protocol(#[from] ProtocolError),

    #[error("network error: {0}")]
    Network(#[from] std::io::Error),

    #[error("connection closed")]
    ConnectionClosed,
}
