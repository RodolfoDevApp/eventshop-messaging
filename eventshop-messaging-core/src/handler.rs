// eventshop-messaging-core/src/handler.rs
use async_trait::async_trait;
use crate::MessagingError;

#[async_trait]
pub trait EventCallback: Send + Sync {
    async fn handle(&self, routing_key: &str, body: &[u8]) -> Result<(), MessagingError>;
}
