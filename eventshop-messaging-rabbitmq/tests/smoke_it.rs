use eventshop_messaging_core::{EventBus, EventCallback, MessagingError};
use eventshop_messaging_rabbitmq::{RabbitEventBus, RabbitMqOptions};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

struct OneShot(Mutex<Option<oneshot::Sender<()>>>);

#[async_trait::async_trait]
impl EventCallback for OneShot {
    async fn handle(&self, _rk: &str, _body: &[u8]) -> Result<(), MessagingError> {
        if let Some(tx) = self.0.lock().unwrap().take() {
            let _ = tx.send(());
        }
        Ok(())
    }
}

#[tokio::test]
async fn publish_and_receive() -> Result<(), Box<dyn std::error::Error>> {
    // Ajusta si usas otras credenciales/host
    let opts = RabbitMqOptions {
        uri: "amqp://admin:admin@localhost:5672/%2f".into(),
        exchange: "eventshop.exchange".into(),
        service: "test".into(),
        durable: true,
        prefetch: 5,
        retry_ttl_ms: 1000,
        confirms: true, // activa publisher confirms en el canal
    };

    let bus = RabbitEventBus::connect(opts).await?;

    let (tx, rx) = oneshot::channel();
    let handler = Arc::new(OneShot(Mutex::new(Some(tx))));

    // Suscribir y dejar que la task declare exchange/queues/bindings
    bus.subscribe("it.q", &["it.#"], handler).await?;
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;

    // Publicar; la RK it.msg hace match con el patron it.#
    bus.publish("it.msg", &serde_json::json!({"ok": true}))
        .await?;

    // Esperar confirmacion de recepcion
    tokio::time::timeout(std::time::Duration::from_secs(5), rx).await??;
    Ok(())
}
