//! Plain domain types shared by every layer. No framework or database types live here.

use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    InferencePerformance,
    ResourceEfficiency,
    ModelQuality,
    Agentic,
    ContextScalability,
}

impl Category {
    pub const ALL: [Category; 5] = [
        Category::InferencePerformance,
        Category::ResourceEfficiency,
        Category::ModelQuality,
        Category::Agentic,
        Category::ContextScalability,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Category::InferencePerformance => "inference_performance",
            Category::ResourceEfficiency => "resource_efficiency",
            Category::ModelQuality => "model_quality",
            Category::Agentic => "agentic",
            Category::ContextScalability => "context_scalability",
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            Category::InferencePerformance => "Inference & Performance",
            Category::ResourceEfficiency => "Resource Efficiency",
            Category::ModelQuality => "Model Intelligence & Quality",
            Category::Agentic => "Agentic Capabilities & Reliability",
            Category::ContextScalability => "Context Window & Scalability",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Category::InferencePerformance => {
                "Generation speed, latency, throughput, and responsiveness"
            }
            Category::ResourceEfficiency => "VRAM, RAM, GPU utilization, power, and offloading",
            Category::ModelQuality => {
                "Reasoning, coding, factual accuracy, instruction following, and language capabilities"
            }
            Category::Agentic => {
                "Tool calling, structured output, task completion, recovery, and multi-step execution"
            }
            Category::ContextScalability => {
                "Long-context accuracy, KV-cache usage, concurrency, and performance degradation"
            }
        }
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Category {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Category::ALL
            .into_iter()
            .find(|c| c.as_str() == s)
            .ok_or_else(|| format!("unknown category '{s}'"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Queued,
    Running,
    Completed,
    Failed,
    /// Stopped by the user before it finished.
    Cancelled,
}

impl RunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RunStatus::Queued => "queued",
            RunStatus::Running => "running",
            RunStatus::Completed => "completed",
            RunStatus::Failed => "failed",
            RunStatus::Cancelled => "cancelled",
        }
    }
}

impl FromStr for RunStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "queued" => Ok(RunStatus::Queued),
            "running" => Ok(RunStatus::Running),
            "completed" => Ok(RunStatus::Completed),
            "failed" => Ok(RunStatus::Failed),
            "cancelled" => Ok(RunStatus::Cancelled),
            other => Err(format!("unknown run status '{other}'")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metric {
    pub name: String,
    pub value: f64,
    pub unit: String,
}

impl Metric {
    pub fn new(name: impl Into<String>, value: f64, unit: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value,
            unit: unit.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub id: Uuid,
    pub name: String,
    pub file_path: String,
    pub architecture: Option<String>,
    pub quantization: Option<String>,
    pub size_label: Option<String>,
    pub block_count: Option<i32>,
    pub file_size_bytes: i64,
    pub created_at: DateTime<Utc>,
}

/// A model discovered on disk, before it is persisted.
#[derive(Debug, Clone)]
pub struct NewModel {
    pub name: String,
    pub file_path: String,
    pub architecture: Option<String>,
    pub quantization: Option<String>,
    pub size_label: Option<String>,
    pub block_count: Option<i32>,
    pub file_size_bytes: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct HardwareProfile {
    pub id: Uuid,
    pub cpu: String,
    pub gpu: Option<String>,
    pub vram_total_bytes: Option<i64>,
    pub ram_total_bytes: i64,
    pub backend: String,
    pub driver_version: Option<String>,
    pub fingerprint: String,
}

#[derive(Debug, Clone)]
pub struct NewHardwareProfile {
    pub cpu: String,
    pub gpu: Option<String>,
    pub vram_total_bytes: Option<i64>,
    pub ram_total_bytes: i64,
    pub backend: String,
    pub driver_version: Option<String>,
}

impl NewHardwareProfile {
    pub fn fingerprint(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}",
            self.cpu,
            self.gpu.as_deref().unwrap_or("-"),
            self.vram_total_bytes.unwrap_or(0),
            self.ram_total_bytes,
            self.backend
        )
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Run {
    pub id: Uuid,
    pub model_id: Uuid,
    pub hardware_id: Uuid,
    pub category: Category,
    pub benchmark_name: String,
    pub status: RunStatus,
    pub config: Value,
    pub details: Option<Value>,
    pub error: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewRun {
    pub model_id: Uuid,
    pub hardware_id: Uuid,
    pub category: Category,
    pub benchmark_name: String,
    pub config: Value,
}

#[derive(Debug, Clone, Default)]
pub struct RunFilter {
    pub model_id: Option<Uuid>,
    pub category: Option<Category>,
    pub benchmark_name: Option<String>,
    pub status: Option<RunStatus>,
    pub limit: Option<i64>,
    /// Rows to skip before `limit`, for paging through older runs.
    pub offset: Option<i64>,
}

/// llama-server settings a model was loaded with.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoadSettings {
    pub ctx_size: u32,
    pub n_gpu_layers: i32,
    pub parallel: u32,
    pub threads: Option<i32>,
}

#[cfg(test)]
mod tests;
