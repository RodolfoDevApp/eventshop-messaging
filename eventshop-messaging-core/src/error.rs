// eventshop-messaging-core/src/error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MessagingError {
    #[error("connection error: {0}")]
    Connection(String),

    #[error("publish error: {0}")]
    Publish(String),

    #[error("subscribe error: {0}")]
    Subscribe(String),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("handler error: {0}")]
    Handler(String),
}
