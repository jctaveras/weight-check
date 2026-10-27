//! Adapters to the outside world (llama-server, hardware sensors, GGUF files).
//! Services call into these; the presentation layer never does.

pub mod gguf;
pub mod llama;
pub mod monitor;
