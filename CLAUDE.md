# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

weight-check is a benchmark tool for local LLMs served by llama.cpp. The monorepo has a Rust backend (`backend/`) and a Next.js frontend (`frontend/`). See the Frontend section below.

## Commands

Run all cargo commands from `backend/`. Cargo is installed via rustup in `~/.cargo/bin`, which may not be on PATH in a fresh shell.

```sh
podman-compose up -d                         # Postgres 17 (repo root; docker compose also works)
cargo run                                    # reads ../.env via dotenvy; listens on 127.0.0.1:8787
cargo test                                   # unit tests (no DB needed)
cargo test idle_watchdog                     # single test by name filter
TEST_DATABASE_URL=postgres://weight:weight@localhost:5432/weight_check_test cargo test --test postgres
cargo fmt && cargo clippy --all-targets -- -D warnings
cargo cov --summary-only                     # coverage via cargo-llvm-cov (alias in .cargo/config.toml)
cargo cov-html                               # HTML report in target/llvm-cov/html
```

- Coverage aliases exclude `main.rs`, `service/testing.rs` and unit test files (`**/tests.rs`). Set `TEST_DATABASE_URL` so that `repository/postgres.rs` is covered too.

- The Postgres integration test (`tests/postgres.rs`) silently skips unless `TEST_DATABASE_URL` is set. It runs migrations against that database.
- Migrations in `backend/migrations/` are applied automatically at startup via `sqlx::migrate!`. Schema changes go in new numbered files; don't edit `0001_init.sql`.

## Architecture

The backend uses a strict layered architecture. Respect the boundaries:

- **`presentation/`** (axum) only parses requests, calls a single service method, and maps the result.
  - No business logic here.
  - Request DTOs live in `dto.rs`.
  - The `ApiJson` and `ApiQuery` extractors turn malformed payloads into `AppError::Validation`.
  - `AppError` → HTTP status mapping is in `presentation/error.rs`.
- **`service/`** holds all logic and depends only on repository **traits** and infra traits, never on sqlx or reqwest types directly.
- **`repository/`** contains the traits (`mod.rs`) and `PgStore` (`postgres.rs`), which implements all of them.
  - Queries are runtime-checked (`query_as`), not `query!` macros, so builds don't need a live DB.
  - Postgres enums (`benchmark_category`, `run_status`) are bound as text with `::benchmark_category` casts and read back with `::text`. The `domain` types stay free of sqlx.
- **`infra/`** holds the adapters the services use:
  - `llama/`: the `ServerLauncher`, `RunningServer` and `LlamaApi` traits, plus the real llama-server process and HTTP client.
  - `monitor/`: `GpuMonitor` (AMD via amdgpu sysfs; this is where NVML would be added) and `HostMonitor` (sysinfo).
  - `gguf.rs`: header metadata reader.
- **`domain/`** contains plain shared types. **`error.rs`** contains the single `AppError` used by every layer.
- **Wiring** happens in `main.rs`: config → pool → `PgStore` → monitors → services → router. `lib.rs` exposes the modules for integration tests.

### Model lifecycle (`service/model_service.rs`)

- weight-check spawns `llama-server` as a child process on a free localhost port, rather than linking llama.cpp through FFI. Load means spawn and wait for `/health`; unload means kill.
- Only one model can be loaded at a time.
- `ModelSession::checkout()` returns the active model plus an `ActivityGuard`. While any guard is alive, explicit unloads return `Busy` and the idle watchdog skips the model.
  - Checkout and unload both run under the `state` mutex, so they cannot interleave.
  - The `lifecycle` tokio mutex serializes load and unload.
- The watchdog and tests use `tokio::time::Instant`, so `#[tokio::test(start_paused = true)]` can fast-forward through idle timeouts.

### Benchmarks (`service/benchmarks/`)

- Each benchmark implements the `Benchmark` trait, which has three methods:
  - `descriptor()` returns the id, category, `implemented` flag and default config.
  - `resolve_config()` validates input and fills in defaults, usually via `resolve_config::<Config>()` with a `#[serde(default, deny_unknown_fields)]` struct.
  - `run()` returns a `BenchmarkOutcome { metrics, details }`.
- Register new benchmarks in `Registry::default()`.
- The five categories come from `domain::Category`. Categories 3–5 are currently `stubs.rs` placeholders that return `NotImplemented` (HTTP 501).
- `BenchmarkService::start_run` handles each run:
  1. Validate the request.
  2. Take the single-run semaphore and a model checkout.
  3. Insert the run as `queued`.
  4. Execute it in a spawned task, which moves it to `running` and then `completed` or `failed`.
  5. Keep the permit and guard alive until the task finishes.
- Requests go through `completion_request()` (temperature 0, `cache_prompt: false`, `ignore_eos: true`), so runs are deterministic and comparable.

### Storage model

The schema is a hybrid:
- **Relational tables:** `models`, `hardware_profiles` and `benchmark_runs`.
- **JSONB columns on runs:**
  - `config` holds `{benchmark: resolved config, server: LoadSettings}`.
  - `details` holds raw, benchmark-specific output such as samples and per-request timings.
- **The narrow `metrics` table:** `(run_id, name, value, unit)`, written in the same transaction that completes the run.
  - Every benchmark should put its headline numbers in `metrics`, so runs can be compared with plain SQL. Anything else goes in `details`.

### Unit test layout

Unit tests never live inline in the file they test. Each module declares `#[cfg(test)] mod tests;` as its last item, and the tests go in a separate file that the compiler resolves on its own (no `#[path]`):
- For a leaf module `foo.rs`, the tests go in `foo/tests.rs`.
- For a `dir/mod.rs` module, the tests go in `dir/tests.rs`.

The tests file is still a child module, so `use super::*;` reaches private items. The top-level `backend/tests/` directory is only for integration tests against the public API.

### Test fakes

`service/testing.rs` (compiled only for tests) provides in-memory repositories (`InMemoryModels`, `InMemoryRuns`, `InMemoryHardware`), `FakeLlama` (fixed 50 tok/s), `FakeSession`, `FakeHost`, `FakeGpu`, `FakeHealth` and `FakeLauncher`. Reuse these when testing services.

Other test setups:
- **Router tests** (`presentation/tests.rs`) build the real router on top of these fakes and drive it with `tower::ServiceExt::oneshot`.
- **`LlamaHttpClient` tests** run against an axum stand-in server on a random port.
- **Launcher tests** use fake `llama-server` shell scripts.
- `infra::gguf::tests::gguf_bytes` builds a minimal valid GGUF header.

## Environment notes

- Configuration comes from `.env` at the repo root (see `.env.example`).
  - Keys: `DATABASE_URL`, `BIND_ADDR`, `LLAMA_SERVER_BIN`, `MODELS_DIR`, `IDLE_TIMEOUT_SECS` (0 disables the watchdog) and `LLAMA_LOAD_TIMEOUT_SECS`.
- The developer's machine often runs its own llama-server on port 8080, holding most of the 24 GB VRAM on the RX 7900 XTX (Vulkan build). Don't kill those processes. When testing, use a small model with partial offload (`n_gpu_layers`), or ask first.

## Frontend (`frontend/`)

Next.js 16 App Router, TypeScript, Tailwind CSS v4, TanStack Query, Vitest + Testing Library. **Use pnpm, not npm.** Run commands from `frontend/`:

```sh
pnpm dev                                      # http://localhost:3000; needs the backend on :8787
pnpm lint && pnpm typecheck && pnpm test && pnpm build
```

- This Next.js version differs from older ones. Read `frontend/AGENTS.md` and the bundled docs in `node_modules/next/dist/docs/` before using unfamiliar APIs.
- `cacheComponents` and `partialPrefetching` are on. Any client component that calls `usePathname`, `useSearchParams` or `useParams` under a dynamic route must sit inside a `<Suspense>` boundary, or the build fails. Hidden routes stay mounted through React `<Activity>`, so form and mutation state survives navigation; reset it explicitly when needed.
- The browser calls the backend directly at `NEXT_PUBLIC_API_URL` (default `http://127.0.0.1:8787`), not through Next rewrites, because a model load can outlast the dev proxy timeout.
- Layers mirror the backend:
  - `lib/api/` holds the fetch client, one function per endpoint and the TS mirrors of the Rust types. Update `types.ts` when backend DTOs change.
  - `lib/queries/` holds the TanStack hooks.
  - `components/ui/` holds the primitives, and `components/<feature>/` the feature components.
  - Pages in `app/` only compose components.
- Every query hook is wrapped in `useHydratedQuery` (`lib/queries/hydration.ts`), so the first client render matches the server's loading state. Do the same for new query hooks.
- Put form and parsing logic in pure modules (e.g. `components/models/loadForm.ts`, `components/benchmarks/configJson.ts`) and unit-test them.
- Tests are `*.test.ts(x)` files next to their source. `src/test/render.tsx` provides `renderWithQuery`, `mockFetch` and `jsonResponse`.
- Follow the Vercel Web Interface Guidelines (`/web-design-guidelines`):
  - labelled inputs, visible `:focus-visible` rings
  - `…` in loading text, `tabular-nums` for numbers, `Intl` formatting
  - Title Case for headings and buttons, error messages that give a next step
  - no browser `confirm()` dialogs
