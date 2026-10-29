# weight-check

A friendly desktop benchmark tool for local LLMs served by [llama.cpp](https://github.com/ggml-org/llama.cpp), built with [Tauri v2](https://v2.tauri.app/).

The app manages `llama-server` as a child process, runs benchmarks against the loaded model, samples hardware telemetry, and stores results in an embedded SQLite database.

| Path | Crate | |
|---|---|---|
| [`core/`](core/) | `weight-check-core` | Domain, services, storage and infra adapters. No Tauri dependency. |
| [`src-tauri/`](src-tauri/) | `weight-check` | The desktop app: wiring plus the Tauri commands the UI calls. |
| [`ui/`](ui/) | | Vite + React frontend loaded by the app's webview. |

## Architecture

```
commands    (src-tauri)    Tauri IPC: argument parsing, error → code mapping. No business logic.
     │
service     (core)         Model lifecycle (load / unload / idle timeout), benchmark registry and
     │                     orchestration, system info. Uses infra adapters for llama-server & sensors.
repository  (core, sqlx)   Traits + SQLite implementation. Stores models, hardware, runs, metrics.
```

- `core/src/infra/` holds the adapters the services use: the `llama-server` launcher and HTTP client, the GGUF metadata reader, and the GPU/host monitors.
- Services depend on repository **traits**, so they are unit-tested with in-memory fakes (exported to the app crate's tests through core's `testing` feature).
- Quitting the app stops the `llama-server` it spawned.

### Database

The schema is a hybrid; see `core/migrations/`:

| Table | Purpose |
|---|---|
| `models` | GGUF files found in `MODELS_DIR`, plus metadata read from the GGUF header (architecture, quantization, layer count) |
| `hardware_profiles` | CPU, GPU, VRAM, RAM and ggml backend, detected at startup |
| `benchmark_runs` | One row per run: category, benchmark, status, and `config` / `details` stored as JSON text for flexible, benchmark-specific data |
| `metrics` | Narrow `(run_id, name, value, unit)` rows holding each run's headline numbers, so results can be compared across runs and categories with plain SQL |

The database file is `weight-check.db` in the OS app data directory (on Linux `~/.local/share/dev.weightcheck.app/`). It is created on first start, and migrations run automatically.

## Benchmarks

| Category | Benchmark id | Status |
|---|---|---|
| Inference & Performance | `inference.generation_speed`, `inference.latency`, `inference.throughput` | ✅ |
| Resource Efficiency | `resource.profile` | ✅ |
| Model Intelligence & Quality | `quality.knowledge`, `quality.math`, `quality.instruction_following`, `quality.coding` | ✅ |
| Agentic Capabilities & Reliability | `agentic.tool_calling` | planned (`not_implemented`) |
| Context Window & Scalability | `context.long_context` | planned (`not_implemented`) |

The `benchmark_catalog` command returns each benchmark's default config. Pass overrides in the `config` field when starting a run.

Quality benchmarks score a seeded sample (`limit`, default 200) of a public dataset:

| Benchmark | Dataset | Score |
|---|---|---|
| `quality.knowledge` | MMLU (5-shot) or ARC-Challenge | `accuracy` from answer-letter probabilities |
| `quality.math` | GSM8K | `accuracy`, exact match on the final number |
| `quality.instruction_following` | IFEval | strict and loose, at the prompt and instruction level |
| `quality.coding` | HumanEval or LiveCodeBench v6 (Jan–Apr 2025) | `pass_at_1` |

Comparing quality runs:
- Datasets are downloaded from Hugging Face on first use (LiveCodeBench is 134 MB), checked against a pinned sha256 and cached in `datasets/` in the app data directory.
- Scores are only comparable between runs with the same benchmark config.
- `quality.coding` runs model-written code on your machine through `PYTHON_BIN`. Each program gets a temp dir, an empty environment and a timeout, but it is not a security sandbox.
- Perplexity is not offered. It is only meaningful between models that share a tokenizer, and llama-server can't score prompt tokens.

GPU telemetry currently supports AMD GPUs through amdgpu sysfs (VRAM, busy %, power). On other GPUs the GPU metrics are omitted.

## Running locally

Requirements: Rust (stable), Node 22+ with pnpm, a built `llama-server`, and the [Tauri system dependencies](https://v2.tauri.app/start/prerequisites/) (on Linux: webkit2gtk-4.1, gtk3, libsoup3). No database server is needed.

```sh
export LLAMA_SERVER_BIN=~/llama.cpp/build/bin/llama-server MODELS_DIR=~/.models   # see .env.example
cd ui && pnpm install && pnpm tauri dev    # starts the Vite dev server, then the app
cd ui && pnpm tauri build                  # release bundle
```

From the app you can load a model, start benchmarks, and watch runs complete with their metrics.

## Commands

The UI talks to the core through Tauri commands (`invoke(name, args)`). Each returns the same JSON the earlier HTTP API returned:

| Command | Arguments | Returns |
|---|---|---|
| `health` | | `{"status": "ok"}` |
| `system_snapshot` | | live CPU/RAM/GPU snapshot |
| `list_models` | | models found in `MODELS_DIR` |
| `model_status` | | loaded model, loading id, active runs, idle time |
| `load_model` | `request: {model_id, ctx_size?, n_gpu_layers?, parallel?, threads?}` | the active model |
| `unload_model` | | `null` |
| `benchmark_catalog` | | benchmarks grouped by category |
| `list_runs` | `filter?: {model_id?, category?, benchmark?, limit?}` | runs, newest first |
| `start_run` | `request: {benchmark_id, config?}` | the queued run |
| `get_run` | `id: "<uuid>"` | the run with its metrics |

Errors reject with `{"code", "message"}`:

| Situation | Code |
|---|---|
| Not found | `not_found` |
| No model loaded | `no_model_loaded` |
| Model busy with a run | `busy` |
| Invalid payload | `validation` |
| Benchmark not implemented yet | `not_implemented` |
| llama-server failure | `llama_server` |
| Database or internal failure | `internal` |

Only one benchmark runs at a time. While a run is active, the model cannot be unloaded and the idle watchdog does not unload it.

## Configuration

Read from the app's process environment at startup (see [`.env.example`](.env.example)):

| Variable | Default | |
|---|---|---|
| `LLAMA_SERVER_BIN` | `llama-server` | path or name on `PATH` |
| `MODELS_DIR` | `./models` | scanned non-recursively; `mmproj` files are skipped |
| `IDLE_TIMEOUT_SECS` | `600` | `0` disables auto-unload |
| `LLAMA_LOAD_TIMEOUT_SECS` | `300` | |
| `PYTHON_BIN` | `python3` | runs model-written code in `quality.coding` |
| `RUST_LOG` | `weight_check_lib=info,weight_check_core=info` | |

## Tests

```sh
cargo test --workspace                                         # unit, command, IPC and SQLite integration tests
cargo clippy --workspace --all-targets -- -D warnings

cd ui
pnpm test                                                      # Vitest component and unit tests
```
