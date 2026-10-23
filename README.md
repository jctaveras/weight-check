# weight-check

A friendly benchmark tool for local LLMs served by [llama.cpp](https://github.com/ggml-org/llama.cpp).

The Rust backend lives in [`backend/`](backend/). It manages `llama-server` as a child process, runs benchmarks against the loaded model, samples hardware telemetry, and stores results in PostgreSQL.

## Architecture

The backend has three layers:

```
presentation  (axum)       HTTP routing, payload parsing, error → status mapping. No business logic.
     │
service                    Model lifecycle (load / unload / idle timeout), benchmark registry and
     │                     orchestration, system info. Uses infra adapters for llama-server & sensors.
repository  (sqlx)         Traits + PostgreSQL implementation. Stores models, hardware, runs, metrics.
```

- `src/infra/` holds the adapters the services use: the `llama-server` launcher and HTTP client, the GGUF metadata reader, and the GPU/host monitors.
- Services depend on repository **traits**, so they are unit-tested with in-memory fakes.

### Database

The schema is a hybrid; see `backend/migrations/`:

| Table | Purpose |
|---|---|
| `models` | GGUF files found in `MODELS_DIR`, plus metadata read from the GGUF header (architecture, quantization, layer count) |
| `hardware_profiles` | CPU, GPU, VRAM, RAM and ggml backend, detected at startup |
| `benchmark_runs` | One row per run: category, benchmark, status, and `config` / `details` stored as JSONB for flexible, benchmark-specific data |
| `metrics` | Narrow `(run_id, name, value, unit)` rows holding each run's headline numbers, so results can be compared across runs and categories with plain SQL |

## Benchmarks

| Category | Benchmark id | Status |
|---|---|---|
| Inference & Performance | `inference.generation_speed`, `inference.latency`, `inference.throughput` | ✅ |
| Resource Efficiency | `resource.profile` | ✅ |
| Model Intelligence & Quality | `quality.reasoning` | planned (501) |
| Agentic Capabilities & Reliability | `agentic.tool_calling` | planned (501) |
| Context Window & Scalability | `context.long_context` | planned (501) |

`GET /benchmarks` returns each benchmark's default config. Pass overrides in the `config` field when starting a run.

GPU telemetry currently supports AMD GPUs through amdgpu sysfs (VRAM, busy %, power). On other GPUs the GPU metrics are omitted.

## Running locally

Requirements: Rust (stable), Podman or Docker, and a built `llama-server`.

```sh
cp .env.example .env          # then edit LLAMA_SERVER_BIN and MODELS_DIR
podman-compose up -d          # or: docker compose up -d
cd backend && cargo run       # listens on 127.0.0.1:8787 by default
```

Migrations run automatically on startup.

## API

```sh
B=localhost:8787
curl $B/health
curl $B/system                                   # live CPU/RAM/GPU snapshot
curl $B/models                                   # scan MODELS_DIR
curl -X POST $B/models/load -H 'content-type: application/json' \
     -d '{"model_id":"<uuid>","ctx_size":4096,"n_gpu_layers":999,"parallel":2}'
curl $B/models/status
curl $B/benchmarks                               # catalog grouped by category
curl -X POST $B/runs -H 'content-type: application/json' \
     -d '{"benchmark_id":"inference.latency","config":{"repetitions":10}}'   # → 202 + run
curl $B/runs/<run-id>                            # status, details and metrics
curl "$B/runs?category=resource_efficiency&model_id=<uuid>&limit=20"
curl -X POST $B/models/unload
```

Error responses use the shape `{"error": {"code", "message"}}`:

| Situation | Status |
|---|---|
| Not found | 404 |
| No model loaded, or busy | 409 |
| Invalid payload | 422 |
| Benchmark not implemented yet | 501 |
| llama-server failure | 502 |

Only one benchmark runs at a time. While a run is active, the model cannot be unloaded and the idle watchdog does not unload it.

## Configuration (`.env`)

| Variable | Default | |
|---|---|---|
| `DATABASE_URL` | — | required |
| `BIND_ADDR` | `127.0.0.1:8787` | |
| `LLAMA_SERVER_BIN` | `llama-server` | path or name on `PATH` |
| `MODELS_DIR` | `./models` | scanned non-recursively; `mmproj` files are skipped |
| `IDLE_TIMEOUT_SECS` | `600` | `0` disables auto-unload |
| `LLAMA_LOAD_TIMEOUT_SECS` | `300` | |

## Tests

```sh
cd backend
cargo test                                                     # unit tests
TEST_DATABASE_URL=postgres://weight:weight@localhost:5432/weight_check_test cargo test   # + Postgres integration
```
