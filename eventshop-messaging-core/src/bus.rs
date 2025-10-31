// eventshop-messaging-core/src/bus.rs
use crate::{EventCallback, MessagingError};
use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
pub trait EventBus: Send + Sync {
    async fn publish<T: serde::Serialize + Send + Sync>(
        &self,
        routing_key: &str,
        payload: &T,
    ) -> Result<(), MessagingError>;

    async fn subscribe(
        &self,
        queue: &str,
        bindings: &[&str],
        handler: Arc<dyn EventCallback>,
    ) -> Result<(), MessagingError>;
}
