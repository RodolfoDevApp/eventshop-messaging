use async_trait::async_trait;
use futures_util::StreamExt;
use lapin::{
    options::*,
    types::{AMQPValue, FieldTable, LongLongInt},
    BasicProperties, Channel, Connection, ConnectionProperties, ExchangeKind,
};
use std::{sync::Arc, time::Duration};
use tokio::{sync::RwLock, time::sleep};
use tracing::{error, info};

use crate::options::{dlq_queue, retry_queue, RabbitMqOptions};
use eventshop_messaging_core::{EventBus, EventCallback, MessagingError};

struct ConnState {
    _conn: Connection,
    pub_ch: Channel,
}

pub struct RabbitEventBus {
    opts: RabbitMqOptions,
    uri: String,
    state: Arc<RwLock<Option<ConnState>>>,
}

impl RabbitEventBus {
    pub async fn connect(opts: RabbitMqOptions) -> Result<Self, MessagingError> {
        let bus = Self {
            uri: opts.uri.clone(),
            opts,
            state: Arc::new(RwLock::new(None)),
        };
        bus.connect_once().await?;
        Ok(bus)
    }

    async fn connect_once(&self) -> Result<(), MessagingError> {
        let conn = Connection::connect(&self.uri, ConnectionProperties::default())
            .await
            .map_err(|e| MessagingError::Connection(e.to_string()))?;

        let ch = conn
            .create_channel()
            .await
            .map_err(|e| MessagingError::Connection(e.to_string()))?;

        ch.exchange_declare(
            &self.opts.exchange,
            ExchangeKind::Topic,
            ExchangeDeclareOptions {
                durable: self.opts.durable,
                auto_delete: false,
                internal: false,
                nowait: false,
                passive: false,
            },
            FieldTable::default(),
        )
        .await
        .map_err(|e| MessagingError::Connection(e.to_string()))?;

        let mut guard = self.state.write().await;
        *guard = Some(ConnState { _conn: conn, pub_ch: ch });

        info!("RabbitMQ connected. exchange={}", self.opts.exchange);
        Ok(())
    }

    async fn current_channel(&self) -> Result<Channel, MessagingError> {
        if let Some(ch) = self
            .state
            .read()
            .await
            .as_ref()
            .map(|s| s.pub_ch.clone())
        {
            return Ok(ch);
        }
        self.connect_once().await?;
        self.state
            .read()
            .await
            .as_ref()
            .map(|s| s.pub_ch.clone())
            .ok_or_else(|| MessagingError::Connection("no channel after reconnect".into()))
    }

    async fn declare_topology_for_queue(
        &self,
        queue: &str,
        bindings: &[&str],
        ch: &Channel,
    ) -> Result<(), MessagingError> {
        let retry = retry_queue(queue);
        let dlq = dlq_queue(queue);

        let mut args = FieldTable::default();
        args.insert(
            "x-dead-letter-exchange".into(),
            AMQPValue::LongString(self.opts.exchange.clone().into()),
        );
        args.insert(
            "x-dead-letter-routing-key".into(),
            AMQPValue::LongString(retry.clone().into()),
        );

        ch.queue_declare(
            queue,
            QueueDeclareOptions {
                durable: self.opts.durable,
                auto_delete: false,
                exclusive: false,
                nowait: false,
                passive: false,
            },
            args,
        )
        .await
        .map_err(|e| MessagingError::Connection(e.to_string()))?;

        let mut retry_args = FieldTable::default();
        retry_args.insert(
            "x-dead-letter-exchange".into(),
            AMQPValue::LongString(self.opts.exchange.clone().into()),
        );
        retry_args.insert(
            "x-dead-letter-routing-key".into(),
            AMQPValue::LongString(queue.into()),
        );
        retry_args.insert(
            "x-message-ttl".into(),
            AMQPValue::LongLongInt(LongLongInt::from(self.opts.retry_ttl_ms as i64)),
        );

        ch.queue_declare(
            &retry,
            QueueDeclareOptions {
                durable: self.opts.durable,
                auto_delete: false,
                exclusive: false,
                nowait: false,
                passive: false,
            },
            retry_args,
        )
        .await
        .map_err(|e| MessagingError::Connection(e.to_string()))?;

        ch.queue_declare(
            &dlq,
            QueueDeclareOptions {
                durable: self.opts.durable,
                auto_delete: false,
                exclusive: false,
                nowait: false,
                passive: false,
            },
            FieldTable::default(),
        )
        .await
        .map_err(|e| MessagingError::Connection(e.to_string()))?;

        for rk in bindings {
            ch.queue_bind(
                queue,
                &self.opts.exchange,
                rk,
                QueueBindOptions { nowait: false },
                FieldTable::default(),
            )
            .await
            .map_err(|e| MessagingError::Connection(e.to_string()))?;
        }

        for rk in [&retry, &dlq] {
            ch.queue_bind(
                rk,
                &self.opts.exchange,
                rk,
                QueueBindOptions { nowait: false },
                FieldTable::default(),
            )
            .await
            .map_err(|e| MessagingError::Connection(e.to_string()))?;
        }

        Ok(())
    }

    fn clone_for_task(&self) -> Self {
        Self {
            opts: self.opts.clone(),
            uri: self.uri.clone(),
            state: Arc::clone(&self.state),
        }
    }
}

#[async_trait]
impl EventBus for RabbitEventBus {
    async fn publish<T: serde::Serialize + Send + Sync>(
        &self,
        routing_key: &str,
        payload: &T,
    ) -> Result<(), MessagingError> {
        let ch = self.current_channel().await?;

        let body = serde_json::to_vec(payload)
            .map_err(|e| MessagingError::Serialization(e.to_string()))?;

        let confirm = ch
            .basic_publish(
                &self.opts.exchange,
                routing_key,
                BasicPublishOptions {
                    mandatory: false,
                    immediate: false,
                },
                &body,
                BasicProperties::default().with_content_type("application/json".into()),
            )
            .await
            .map_err(|e| MessagingError::Publish(e.to_string()))?
            .await
            .map_err(|e| MessagingError::Publish(e.to_string()))?;

        if confirm.is_nack() {
            return Err(MessagingError::Publish(
                "publisher confirm NACK".to_string(),
            ));
        }
        Ok(())
    }

    async fn subscribe(
        &self,
        queue: &str,
        bindings: &[&str],
        handler: Arc<dyn EventCallback>,
    ) -> Result<(), MessagingError> {
        // Propios para 'static en la task
        let queue_owned = queue.to_string();
        let bindings_owned: Vec<String> = bindings.iter().map(|s| s.to_string()).collect();
        let handler_owned = Arc::clone(&handler);
        let bus = self.clone_for_task();

        tokio::spawn(async move {
            loop {
                let ch = match bus.current_channel().await {
                    Ok(c) => c,
                    Err(e) => {
                        error!("subscribe: no channel yet: {e}");
                        sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                };

                if let Err(e) = ch
                    .basic_qos(bus.opts.prefetch, BasicQosOptions { global: false })
                    .await
                {
                    error!("basic_qos failed: {e}");
                    sleep(Duration::from_secs(1)).await;
                    continue;
                }

                // convertimos Vec<String> -> Vec<&str> para la declaracion
                let binding_refs: Vec<&str> = bindings_owned.iter().map(|s| s.as_str()).collect();
                if let Err(e) = bus
                    .declare_topology_for_queue(&queue_owned, &binding_refs, &ch)
                    .await
                {
                    error!("declare_topology_for_queue failed: {e}");
                    sleep(Duration::from_secs(1)).await;
                    continue;
                }

                let consumer = match ch
                    .basic_consume(
                        &queue_owned,
                        &format!("consumer-{}", bus.opts.service),
                        BasicConsumeOptions {
                            no_ack: false,
                            exclusive: false,
                            nowait: false,
                            ..Default::default()
                        },
                        FieldTable::default(),
                    )
                    .await
                {
                    Ok(c) => c,
                    Err(e) => {
                        error!("basic_consume failed: {e}");
                        sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                };

                info!(
                    "Consuming queue={} exchange={}",
                    queue_owned, bus.opts.exchange
                );

                let mut stream = consumer;
                while let Some(delivery) = stream.next().await {
                    match delivery {
                        Ok(d) => {
                            // routing_key es campo, NO metodo
                            let rk = d.routing_key.to_string();
                            let data = d.data.clone();
                            match handler_owned.handle(&rk, &data).await {
                                Ok(_) => {
                                    let _ = d.ack(BasicAckOptions { multiple: false }).await;
                                }
                                Err(err) => {
                                    error!("handler error: {}, routing_key={}", err, rk);
                                    let _ = d
                                        .nack(BasicNackOptions {
                                            multiple: false,
                                            requeue: false,
                                        })
                                        .await;
                                }
                            }
                        }
                        Err(e) => {
                            error!("delivery error: {e}");
                            break;
                        }
                    }
                }

                sleep(Duration::from_secs(1)).await;
            }
        });

        Ok(())
    }
}
