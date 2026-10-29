//! Category 2 — Resource Efficiency.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::inference::DEFAULT_PROMPTS;
use crate::domain::{Category, Metric};
use crate::error::{AppError, AppResult};
use crate::infra::monitor::{GpuMonitor, HostMonitor};
use crate::service::benchmarks::{
    Benchmark, BenchmarkContext, BenchmarkDescriptor, BenchmarkOutcome, completion_request,
    default_config, parse_config, resolve_config, stats,
};

const MIB: f64 = 1024.0 * 1024.0;

/// Samples VRAM, GPU utilization, power and RAM while the model generates.
pub struct ResourceProfile;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub sample_interval_ms: u64,
    /// Sampling time before the workload, to measure the loaded-but-idle baseline.
    pub idle_ms: u64,
    pub prompt: String,
    pub n_predict: u32,
    pub repetitions: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sample_interval_ms: 200,
            idle_ms: 2000,
            prompt: DEFAULT_PROMPTS[0].to_string(),
            n_predict: 512,
            repetitions: 2,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct Sample {
    t_ms: f64,
    idle: bool,
    vram_used_bytes: Option<u64>,
    gpu_busy_pct: Option<f64>,
    power_w: Option<f64>,
    ram_used_bytes: u64,
    process_rss_bytes: Option<u64>,
}

#[async_trait]
impl Benchmark for ResourceProfile {
    fn descriptor(&self) -> BenchmarkDescriptor {
        BenchmarkDescriptor {
            id: "resource.profile",
            category: Category::ResourceEfficiency,
            name: "Resource profile",
            description: "VRAM/RAM footprint, GPU utilization, power draw, energy per token and layer offloading during generation",
            implemented: true,
            default_config: default_config::<Config>(),
        }
    }

    fn resolve_config(&self, raw: Option<Value>) -> AppResult<Value> {
        let value = resolve_config::<Config>(raw)?;
        let config: Config = parse_config(value.clone())?;
        if config.sample_interval_ms < 50 || config.repetitions == 0 || config.n_predict == 0 {
            return Err(AppError::Validation(
                "sample_interval_ms must be >= 50; repetitions and n_predict must be positive"
                    .into(),
            ));
        }
        Ok(value)
    }

    async fn run(&self, ctx: &BenchmarkContext, config: Value) -> AppResult<BenchmarkOutcome> {
        let config: Config = parse_config(config)?;

        let idle = Arc::new(AtomicBool::new(true));
        let stop = StopOnDrop(Arc::new(AtomicBool::new(false)));
        let sampler = tokio::spawn(sample_loop(
            ctx.gpu.clone(),
            ctx.host.clone(),
            ctx.model.pid,
            Duration::from_millis(config.sample_interval_ms),
            idle.clone(),
            stop.0.clone(),
        ));

        tokio::time::sleep(Duration::from_millis(config.idle_ms)).await;
        idle.store(false, Ordering::SeqCst);

        let workload_started = Instant::now();
        let mut workload: AppResult<f64> = Ok(0.0);
        for rep in 0..config.repetitions {
            let req = completion_request(&config.prompt, config.n_predict, rep as i64);
            match ctx.llama.completion(&req).await {
                Ok(resp) => {
                    if let Ok(tokens) = workload.as_mut() {
                        *tokens += resp.timings.predicted_n;
                    }
                }
                Err(e) => {
                    workload = Err(e);
                    break;
                }
            }
        }
        let workload_s = workload_started.elapsed().as_secs_f64();
        drop(stop);
        let samples = sampler
            .await
            .map_err(|e| AppError::Internal(format!("sampler task failed: {e}")))?;
        let tokens = workload?;

        let mut metrics = vec![
            Metric::new("load_time_ms", ctx.model.load_time_ms, "ms"),
            Metric::new("generation_tps", tokens / workload_s, "tok/s"),
        ];
        metrics.extend(footprint_metrics(ctx));
        metrics.extend(sample_metrics(&samples, tokens));

        Ok(BenchmarkOutcome {
            metrics,
            details: json!({
                "tokens_generated": tokens,
                "workload_s": workload_s,
                "gpu": ctx.gpu.as_ref().map(|g| g.name()),
                "settings": ctx.model.settings,
                "block_count": ctx.model.model.block_count,
                "samples": samples,
            }),
        })
    }
}

/// Stops the sampler when dropped, including when a cancelled run drops `run` mid-workload.
struct StopOnDrop(Arc<AtomicBool>);

impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

async fn sample_loop(
    gpu: Option<Arc<dyn GpuMonitor>>,
    host: Arc<dyn HostMonitor>,
    pid: Option<u32>,
    interval: Duration,
    idle: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
) -> Vec<Sample> {
    let started = Instant::now();
    let mut ticker = tokio::time::interval(interval);
    let mut samples = Vec::new();
    while !stop.load(Ordering::SeqCst) {
        ticker.tick().await;
        let gpu = gpu.as_ref().and_then(|g| g.snapshot());
        let host = host.snapshot(pid);
        samples.push(Sample {
            t_ms: started.elapsed().as_secs_f64() * 1000.0,
            idle: idle.load(Ordering::SeqCst),
            vram_used_bytes: gpu.as_ref().map(|g| g.vram_used_bytes),
            gpu_busy_pct: gpu.as_ref().and_then(|g| g.busy_pct),
            power_w: gpu.as_ref().and_then(|g| g.power_w),
            ram_used_bytes: host.ram_used_bytes,
            process_rss_bytes: host.process_rss_bytes,
        });
    }
    samples
}

/// Static footprint: VRAM claimed by loading the model and how many layers were offloaded.
fn footprint_metrics(ctx: &BenchmarkContext) -> Vec<Metric> {
    let mut metrics = Vec::new();
    if let (Some(before), Some(after)) = (
        ctx.model.vram_before_load_bytes,
        ctx.model.vram_after_load_bytes,
    ) {
        let delta = after.saturating_sub(before) as f64 / MIB;
        metrics.push(Metric::new("model_vram_mb", delta, "MiB"));
    }
    if let Some(blocks) = ctx.model.model.block_count {
        // llama.cpp counts the output layer as one extra offloadable layer.
        let offloadable = blocks + 1;
        let offloaded = ctx.model.settings.n_gpu_layers.min(offloadable);
        metrics.push(Metric::new(
            "gpu_layers_offloaded",
            offloaded as f64,
            "layers",
        ));
        metrics.push(Metric::new(
            "offload_pct",
            offloaded as f64 / offloadable as f64 * 100.0,
            "%",
        ));
    }
    metrics
}

fn sample_metrics(samples: &[Sample], tokens: f64) -> Vec<Metric> {
    let busy: Vec<&Sample> = samples.iter().filter(|s| !s.idle).collect();
    let idle: Vec<&Sample> = samples.iter().filter(|s| s.idle).collect();
    let collect = |set: &[&Sample], f: fn(&Sample) -> Option<f64>| -> Vec<f64> {
        set.iter().filter_map(|s| f(s)).collect()
    };

    let mut metrics = Vec::new();
    let mut push = |name: &str, unit: &str, value: Option<f64>| {
        if let Some(v) = value {
            metrics.push(Metric::new(name, v, unit));
        }
    };

    let vram = stats::summarize(&collect(&busy, |s| {
        s.vram_used_bytes.map(|b| b as f64 / MIB)
    }));
    push("vram_peak_mb", "MiB", vram.map(|s| s.max));
    push("vram_avg_mb", "MiB", vram.map(|s| s.mean));

    let util = stats::summarize(&collect(&busy, |s| s.gpu_busy_pct));
    push("gpu_util_avg_pct", "%", util.map(|s| s.mean));
    push("gpu_util_peak_pct", "%", util.map(|s| s.max));

    let power = stats::summarize(&collect(&busy, |s| s.power_w));
    push("power_avg_w", "W", power.map(|s| s.mean));
    push("power_peak_w", "W", power.map(|s| s.max));
    push(
        "power_idle_avg_w",
        "W",
        stats::summarize(&collect(&idle, |s| s.power_w)).map(|s| s.mean),
    );

    let energy = energy_joules(&busy);
    push("energy_j", "J", energy);
    push(
        "tokens_per_joule",
        "tok/J",
        energy.filter(|e| *e > 0.0).map(|e| tokens / e),
    );

    let ram = stats::summarize(&collect(&busy, |s| Some(s.ram_used_bytes as f64 / MIB)));
    push("ram_used_peak_mb", "MiB", ram.map(|s| s.max));
    let rss = stats::summarize(&collect(&busy, |s| {
        s.process_rss_bytes.map(|b| b as f64 / MIB)
    }));
    push("process_rss_peak_mb", "MiB", rss.map(|s| s.max));

    metrics
}

/// Trapezoidal integral of power over time.
fn energy_joules(samples: &[&Sample]) -> Option<f64> {
    let points: Vec<(f64, f64)> = samples
        .iter()
        .filter_map(|s| s.power_w.map(|p| (s.t_ms / 1000.0, p)))
        .collect();
    if points.len() < 2 {
        return None;
    }
    Some(
        points
            .windows(2)
            .map(|w| (w[1].0 - w[0].0) * (w[0].1 + w[1].1) / 2.0)
            .sum(),
    )
}

#[cfg(test)]
mod tests;
