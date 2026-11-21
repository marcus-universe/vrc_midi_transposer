//! MQTT client and Home Assistant integration.

mod service;
pub mod topics;

pub use service::{MqttEvent, MqttHandle};
