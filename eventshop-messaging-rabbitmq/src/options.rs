#[derive(Clone, Debug)]
pub struct RabbitMqOptions {
    pub uri: String,
    pub exchange: String,
    pub service: String,
    pub durable: bool,
    pub prefetch: u16,
    pub retry_ttl_ms: u32,
    /// Si true, activa publisher confirms y espera el ACK/NACK del broker.
    pub confirms: bool,
}

/// Nombres convencionales para colas retry y dlq basadas en el nombre de la cola principal.
pub fn retry_queue(queue: &str) -> String {
    format!("{}.retry", queue)
}
pub fn dlq_queue(queue: &str) -> String {
    format!("{}.dlq", queue)
}
