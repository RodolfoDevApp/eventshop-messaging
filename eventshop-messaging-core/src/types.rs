// eventshop-messaging-core/src/types.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Marker: Command
pub trait Command: Send + Sync + 'static {}

/// Marker: Event
pub trait Event: Send + Sync + 'static {}

/// Base message envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    bound(
        serialize = "TPayload: Serialize",
        deserialize = "TPayload: serde::de::Deserialize<'de>"
    )
)]
pub struct Message<TPayload>
where
    TPayload: Send + Sync + 'static,
{
    pub id: Uuid,
    pub type_name: String,
    pub payload: TPayload,
    pub occurred_at_utc: DateTime<Utc>,
}

/// Integration event envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    bound(
        serialize = "TEvent: Serialize",
        deserialize = "TEvent: serde::de::Deserialize<'de>"
    )
)]
pub struct IntegrationEventEnvelope<TEvent>
where
    TEvent: Event + Send + Sync + 'static,
{
    pub event_id: Uuid,
    pub event_type: String,
    pub event: TEvent,
    pub occurred_at_utc: DateTime<Utc>,
    pub service: String,
    pub version: String,
}

impl<TEvent> IntegrationEventEnvelope<TEvent>
where
    TEvent: Event + Send + Sync + 'static + Serialize,
{
    pub fn new(service: &str, version: &str, event_type: &str, event: TEvent) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            event_type: event_type.to_string(),
            event,
            occurred_at_utc: Utc::now(),
            service: service.to_string(),
            version: version.to_string(),
        }
    }
}

/// Helper to derive routing key from the type name
pub fn default_routing_key(type_name: &str) -> String {
    type_name.to_string()
}
