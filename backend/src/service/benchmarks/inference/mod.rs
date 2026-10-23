//! Category 1 — Inference & Performance.

mod generation_speed;
mod latency;
mod throughput;

pub use generation_speed::GenerationSpeed;
pub use latency::Latency;
pub use throughput::Throughput;

pub(crate) const DEFAULT_PROMPTS: [&str; 3] = [
    "Explain, step by step, how a transformer neural network turns a sentence into a prediction.",
    "Write a short story about a lighthouse keeper who discovers a message in a bottle.",
    "List practical tips for someone learning their first programming language, with examples.",
];
