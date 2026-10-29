//! Adapters to the outside world (llama-server, hardware sensors, GGUF files, eval datasets,
//! the Python runner for generated code).
//! Services call into these; the presentation layer never does.

pub mod datasets;
pub mod gguf;
pub mod llama;
pub mod monitor;
pub mod sandbox;
