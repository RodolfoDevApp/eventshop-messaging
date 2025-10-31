// eventshop-messaging-core/src/bus.rs
use std::sync::Arc;
use async_trait::async_trait;
use crate::{MessagingError, EventCallback};

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
