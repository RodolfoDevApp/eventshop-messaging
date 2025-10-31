// eventshop-messaging-core/src/handler.rs
use crate::MessagingError;
use async_trait::async_trait;

#[async_trait]
pub trait EventCallback: Send + Sync {
    async fn handle(&self, routing_key: &str, body: &[u8]) -> Result<(), MessagingError>;
}
