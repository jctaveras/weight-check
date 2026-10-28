//! Placeholders for categories 3–5 so the catalog already lists every category.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use super::{Benchmark, BenchmarkContext, BenchmarkDescriptor, BenchmarkOutcome};
use crate::domain::Category;
use crate::error::{AppError, AppResult};

pub struct Planned {
    id: &'static str,
    category: Category,
    name: &'static str,
    description: &'static str,
}

pub fn all() -> Vec<Arc<dyn Benchmark>> {
    vec![
        Arc::new(Planned {
            id: "quality.reasoning",
            category: Category::ModelQuality,
            name: "Reasoning & knowledge",
            description: "Reasoning, coding, factual accuracy, instruction following and language tasks",
        }),
        Arc::new(Planned {
            id: "agentic.tool_calling",
            category: Category::Agentic,
            name: "Tool calling & structured output",
            description: "Tool-call correctness, JSON-schema adherence, multi-step task completion and recovery",
        }),
        Arc::new(Planned {
            id: "context.long_context",
            category: Category::ContextScalability,
            name: "Long-context scaling",
            description: "Needle-in-a-haystack accuracy, KV-cache growth and speed degradation as context grows",
        }),
    ]
}

#[async_trait]
impl Benchmark for Planned {
    fn descriptor(&self) -> BenchmarkDescriptor {
        BenchmarkDescriptor {
            id: self.id,
            category: self.category,
            name: self.name,
            description: self.description,
            implemented: false,
            default_config: Value::Null,
        }
    }

    fn resolve_config(&self, _raw: Option<Value>) -> AppResult<Value> {
        Err(AppError::NotImplemented(format!("benchmark '{}'", self.id)))
    }

    async fn run(&self, _ctx: &BenchmarkContext, _config: Value) -> AppResult<BenchmarkOutcome> {
        Err(AppError::NotImplemented(format!("benchmark '{}'", self.id)))
    }
}
