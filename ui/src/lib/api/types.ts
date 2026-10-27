// TypeScript mirrors of the backend's serialized types (backend/src/domain, service/*).

export type Uuid = string;
export type IsoDateTime = string;

export type Category =
  | "inference_performance"
  | "resource_efficiency"
  | "model_quality"
  | "agentic"
  | "context_scalability";

export type RunStatus = "queued" | "running" | "completed" | "failed";

export type ErrorCode =
  | "not_found"
  | "no_model_loaded"
  | "busy"
  | "validation"
  | "not_implemented"
  | "llama_server"
  | "internal";

export interface ErrorEnvelope {
  error: { code: ErrorCode; message: string };
}

export interface ModelInfo {
  id: Uuid;
  name: string;
  file_path: string;
  architecture: string | null;
  quantization: string | null;
  size_label: string | null;
  block_count: number | null;
  file_size_bytes: number;
  created_at: IsoDateTime;
}

export interface LoadSettings {
  ctx_size: number;
  n_gpu_layers: number;
  parallel: number;
  threads: number | null;
}

export interface ActiveModel {
  model: ModelInfo;
  settings: LoadSettings;
  pid: number | null;
  load_time_ms: number;
  vram_before_load_bytes: number | null;
  vram_after_load_bytes: number | null;
  loaded_at: IsoDateTime;
}

export interface ModelStatus {
  loaded: ActiveModel | null;
  loading_model_id: Uuid | null;
  active_runs: number;
  idle_secs: number;
  idle_timeout_secs: number | null;
}

export interface LoadModelRequest {
  model_id: Uuid;
  ctx_size?: number;
  n_gpu_layers?: number;
  parallel?: number;
  threads?: number;
}

export interface HostSnapshot {
  cpu: string;
  cpu_cores: number;
  cpu_usage_pct: number;
  ram_total_bytes: number;
  ram_used_bytes: number;
  process_rss_bytes: number | null;
}

export interface GpuSnapshot {
  name: string;
  vram_used_bytes: number;
  vram_total_bytes: number;
  busy_pct: number | null;
  power_w: number | null;
}

export interface SystemSnapshot {
  os: string | null;
  backend: string;
  host: HostSnapshot;
  gpu: GpuSnapshot | null;
}

export type JsonValue =
  | string
  | number
  | boolean
  | null
  | JsonValue[]
  | { [key: string]: JsonValue };

export interface BenchmarkDescriptor {
  id: string;
  category: Category;
  name: string;
  description: string;
  implemented: boolean;
  default_config: JsonValue;
}

export interface CategoryCatalog {
  category: Category;
  title: string;
  description: string;
  benchmarks: BenchmarkDescriptor[];
}

export interface StartRunRequest {
  benchmark_id: string;
  config?: JsonValue;
}

export interface Metric {
  name: string;
  value: number;
  unit: string;
}

export interface Run {
  id: Uuid;
  model_id: Uuid;
  hardware_id: Uuid;
  category: Category;
  benchmark_name: string;
  status: RunStatus;
  config: JsonValue;
  details: JsonValue | null;
  error: string | null;
  started_at: IsoDateTime | null;
  finished_at: IsoDateTime | null;
  created_at: IsoDateTime;
}

export interface RunWithMetrics extends Run {
  metrics: Metric[];
}

export interface RunFilter {
  model_id?: Uuid;
  category?: Category;
  benchmark?: string;
  limit?: number;
}
