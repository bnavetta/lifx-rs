use lifx_proto::{wire::WireError, LifxError};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// We ran out of sequence numbers. This happens if there are too many outstanding messages that require a response or acknowledgement.
    #[error("ran out of sequence numbers")]
    SequenceExhausted,

    #[error("protocol error: {0}")]
    Protocol(#[from] LifxError),

    #[error("network error: {0}")]
    Network(#[from] std::io::Error),

    #[error("connection closed")]
    ConnectionClosed,
}

impl From<WireError> for Error {
    fn from(err: WireError) -> Error {
        Error::Protocol(LifxError::Wire(err))
    }
}
