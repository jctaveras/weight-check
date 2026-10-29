# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

weight-check is a desktop benchmark tool for local LLMs served by llama.cpp, built with Tauri v2. The monorepo is a Cargo workspace with two crates, plus the UI:
- `core/` (`weight-check-core`, lib `weight_check_core`): domain, services, storage and infra adapters. No Tauri dependency.
- `src-tauri/` (`weight-check`, lib `weight_check_lib`): the desktop app. Wires the core services together and exposes them to the UI as Tauri commands.
- `ui/`: the Vite + React frontend that the app's webview loads. See the Frontend section below.

## Commands

Run cargo commands from the repo root (the workspace root; there is a single `target/` there). Cargo is installed via rustup in `~/.cargo/bin`, which may not be on PATH in a fresh shell.

```sh
cd ui && pnpm tauri dev                      # launch the app (runs `pnpm dev` in ui/, then cargo run)
cd ui && pnpm tauri build                    # release bundle (runs `pnpm build` in ui/)
cargo build -p weight-check                  # build the app binary only (debug builds load devUrl)
cargo test --workspace                       # unit tests + SQLite integration tests
cargo test --workspace idle_watchdog         # single test by name filter
cargo test -p weight-check-core --test sqlite  # repository integration tests only
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo cov --summary-only                     # coverage via cargo-llvm-cov (alias in .cargo/config.toml)
cargo cov-html                               # HTML report in target/llvm-cov/html
```

- The Tauri CLI comes from `ui/`'s devDependencies (`@tauri-apps/cli`). Tauri runs `beforeDevCommand`/`beforeBuildCommand` (`pnpm dev`/`pnpm build`) with `ui/` as the working directory. From `ui/` the CLI only finds `src-tauri/` through `TAURI_APP_PATH=../src-tauri` (the `tauri` script in `ui/package.json` sets it via `cross-env`); from the repo root it is found automatically (`ui/node_modules/.bin/tauri dev`).
- Coverage aliases cover the whole workspace and exclude `src-tauri/src/main.rs`, `service/testing.rs` and unit test files (`**/tests.rs`). `repository/sqlite.rs` is covered by the integration tests.
- The SQLite integration test (`core/tests/sqlite.rs`) always runs: each test opens a fresh database file in a `tempfile` dir via `SqliteStore::connect`/`open`, which apply the migrations.
- The database is an embedded SQLite file, `weight-check.db` in the OS app data directory (`app.path().app_data_dir()`, e.g. `~/.local/share/dev.weightcheck.app/` on Linux), created if missing and opened with foreign keys on and WAL journaling.
- Migrations in `core/migrations/` are applied automatically at startup via `sqlx::migrate!` (inside `SqliteStore::connect`/`open`). Schema changes go in new numbered files; never edit an applied one.
- `0002_run_cancelled_status.sql` rebuilds `benchmark_runs` and `metrics` to allow the `cancelled` status. Changing a CHECK constraint means rebuilding the table, and the child table must be rebuilt first: dropping a parent that still has `ON DELETE CASCADE` children deletes their rows.

## Architecture

The Rust code uses a strict layered architecture. Respect the boundaries:

- **`src-tauri/src/commands/`** (Tauri IPC) only parses arguments, calls a single service method, and maps the result. It replaces the old axum `presentation/` layer with the same rules.
  - No business logic here.
  - Each command is a plain `pub async fn x(state: &AppState, ...)` in `commands/mod.rs`. The `#[tauri::command]` wrappers in `commands/ipc.rs` only unwrap `tauri::State` and delegate (all async, so none runs on the main thread). `ipc::handler()` is the `generate_handler!` list; register new commands there.
  - Command names and Rust params are snake_case (JS sends camelCase keys; Tauri maps them). Each command returns exactly the JSON the old HTTP endpoint returned, so the UI's types match the Rust DTOs.
  - Request DTOs live in `commands/dto.rs`. Payload arguments are taken as `serde_json::Value` and decoded with `dto::parse`, so malformed payloads become `AppError::Validation` (Tauri's own argument errors would reach the UI as bare strings).
  - `AppError` → `CommandError` (`{"code", "message"}`) mapping is in `commands/error.rs`. Codes: `not_found`, `no_model_loaded`, `busy`, `validation`, `not_implemented`, `llama_server`, `internal`.
- **`core/src/service/`** holds all logic and depends only on repository **traits** and infra traits, never on sqlx or reqwest types directly.
- **`core/src/repository/`** contains the traits (`mod.rs`) and `SqliteStore` (`sqlite.rs`), which implements all of them.
  - Queries are runtime-checked (`query_as`), not `query!` macros, so builds don't need a live DB.
  - SQLite has no enum, UUID, timestamp or JSON types. Category and status are TEXT columns guarded by `CHECK (... IN (...))`; UUIDs are bound and read as hyphenated TEXT (`uuid::fmt::Hyphenated`); timestamps are bound from Rust (`Utc::now()`, never SQL `now()`) as RFC 3339 TEXT; JSON goes through `sqlx::types::Json`. Placeholders are `?N`. The `domain` types stay free of sqlx.
- **`core/src/infra/`** holds the adapters the services use:
  - `llama/`: the `ServerLauncher`, `RunningServer` and `LlamaApi` traits, plus the real llama-server process and HTTP client (`/completion`, optionally with `n_probs`, and `/v1/chat/completions`).
  - `monitor/`: `GpuMonitor` (AMD via amdgpu sysfs; this is where NVML would be added) and `HostMonitor` (sysinfo).
  - `gguf.rs`: header metadata reader.
  - `datasets.rs`: `DatasetStore` (`HttpDatasetStore` downloads a pinned Hugging Face file, checks its sha256 and caches it under `<app data>/datasets/<repo>/<revision>/<path>`), plus `read_parquet`/`read_jsonl`.
  - `sandbox.rs`: `CodeRunner` (`PythonRunner`: `python3 -I` in a fresh temp dir with an empty env and a timeout). It isolates runs from each other but is not a security sandbox.
- **`core/src/domain/`** contains plain shared types. **`core/src/error.rs`** contains the single `AppError` used by every layer. **`core/src/config.rs`** reads `LLAMA_SERVER_BIN`, `MODELS_DIR`, `IDLE_TIMEOUT_SECS`, `LLAMA_LOAD_TIMEOUT_SECS` and `PYTHON_BIN` from the process env (temporary; Phase 4 replaces it with in-app settings).
- **Wiring** happens in `src-tauri/src/lib.rs`: `run()` builds the Tauri app; its `setup` hook `block_on`s `init()` (config → `build_state(config, db_path, datasets_dir)`: `SqliteStore::open` (pool + migrations) → monitors → services → `spawn_idle_watchdog()`) and `app.manage(AppState { models, benchmarks, system })`. On `RunEvent::Exit` it calls `shutdown()`, which stops any loaded llama-server (even mid-run) so it never outlives the app. `main.rs` only calls `weight_check_lib::run()`.

### Model lifecycle (`core/src/service/model_service.rs`)

- weight-check spawns `llama-server` as a child process on a free localhost port, rather than linking llama.cpp through FFI. Load means spawn and wait for `/health`; unload means kill.
- Only one model can be loaded at a time.
- `ModelSession::checkout()` returns the active model plus an `ActivityGuard`. While any guard is alive, explicit unloads return `Busy` and the idle watchdog skips the model.
  - Checkout and unload both run under the `state` mutex, so they cannot interleave.
  - The `lifecycle` tokio mutex serializes load and unload.
- The watchdog and tests use `tokio::time::Instant`, so `#[tokio::test(start_paused = true)]` can fast-forward through idle timeouts.

### Benchmarks (`core/src/service/benchmarks/`)

- Each benchmark implements the `Benchmark` trait, which has three methods:
  - `descriptor()` returns the id, category, `implemented` flag and default config.
  - `resolve_config()` validates input and fills in defaults, usually via `resolve_config::<Config>()` with a `#[serde(default, deny_unknown_fields)]` struct.
  - `run()` returns a `BenchmarkOutcome { metrics, details }`.
- Register new benchmarks in `Registry::standard()`. It takes `QualityDeps { datasets, code_runner }`, which the quality benchmarks hold; tests pass `fake_quality_deps()`.
- The five categories come from `domain::Category`. Categories 4–5 are currently `stubs.rs` placeholders that return `NotImplemented` (error code `not_implemented`).
- `BenchmarkService::start_run` handles each run:
  1. Validate the request.
  2. Take the single-run semaphore and a model checkout.
  3. Insert the run as `queued`.
  4. Execute it in a spawned task, which moves it to `running` and then `completed` or `failed`.
  5. Keep the permit and guard alive until the benchmark future ends.
- `cancel_run(id)` signals the active run's task through a `oneshot`.
  - The task `select!`s the benchmark against it and drops the benchmark future on cancel. Dropping aborts reqwest calls; child processes use `kill_on_drop`; spawned helpers need a drop guard, like `resource::StopOnDrop`.
  - The task then frees the gate, the guard and the `active` slot before it stores the final status, so a client that sees the final status can start the next run straight away.
  - An unfinished run with no task behind it (a repeated cancel, or a run orphaned by a crash) is marked `cancelled` directly.
- `delete_run` refuses the active run (`Busy`).
- Final statuses (`completed`, `failed`, `cancelled`) are never overwritten. The SQLite `mark_running`/`complete`/`fail`/`cancel` updates are guarded by status (`UNFINISHED`), and `InMemoryRuns` mirrors this.
- `FakeLlama` yields on every request, as real I/O does, so cancellation works in tests.
- Requests go through `completion_request()` (temperature 0, `cache_prompt: false`, `ignore_eos: true`), so runs are deterministic and comparable.

### Quality benchmarks (`core/src/service/benchmarks/quality/`)

- There are four benchmarks: `quality.knowledge` (MMLU or ARC-Challenge), `quality.math` (GSM8K), `quality.instruction_following` (IFEval) and `quality.coding` (HumanEval or LiveCodeBench v6).
- **Datasets** are pinned in `sources.rs` by revision and sha256, and loaded with `load_rows`, which picks the reader by file extension.
  - Bumping a pin changes what a benchmark measures, so runs before and after the bump are not comparable.
- **Item selection** is deterministic. `sample`/`sample_stratified` use a seeded SplitMix64 shuffle (pinned by a test) and return items in dataset order.
- **Requests:**
  - `knowledge` scores the answer-letter log-probabilities from a raw `/completion` (`n_predict: 1`, `n_probs: 20`).
  - The other three use chat (`chat_request`, temperature 0, optional `enable_thinking: false`).
  - A `finish_reason` of `length` counts as invalid.
  - Requests fan out through `eval_all`, capped at the server's `parallel` slots.
- **IFEval checks** (`ifeval/checks.rs`) are ported from Google's reference implementation. Words and sentences are split by regex rather than NLTK, and `language:response_language` is skipped and reported in `details.skipped_instructions`.
- **LiveCodeBench** private tests are base64(zlib(pickled str)). `pickled_str` reads the string's bytes directly and never unpickles.

### Storage model

The schema is a hybrid:
- **Relational tables:** `models`, `hardware_profiles` and `benchmark_runs`.
- **JSON (TEXT, `json_valid`-checked) columns on runs:**
  - `config` holds `{benchmark: resolved config, server: LoadSettings}`.
  - `details` holds raw, benchmark-specific output such as samples and per-request timings.
- **The narrow `metrics` table:** `(run_id, name, value, unit)`, written in the same transaction that completes the run.
  - Every benchmark should put its headline numbers in `metrics`, so runs can be compared with plain SQL. Anything else goes in `details`.

### Unit test layout

Unit tests never live inline in the file they test. Each module declares `#[cfg(test)] mod tests;` as its last item, and the tests go in a separate file that the compiler resolves on its own (no `#[path]`):
- For a leaf module `foo.rs`, the tests go in `foo/tests.rs`.
- For a `dir/mod.rs` module (or a crate root `lib.rs`), the tests go in `dir/tests.rs` (`src/tests.rs`).

The tests file is still a child module, so `use super::*;` reaches private items. The top-level `core/tests/` directory is only for integration tests against the public API.

### Test fakes

`core/src/service/testing.rs` provides in-memory repositories (`InMemoryModels`, `InMemoryRuns`, `InMemoryHardware`), `FakeLlama` (fixed 50 tok/s), `FakeSession`, `FakeHost`, `FakeGpu`, `FakeHealth`, `FakeLauncher` and `gguf_bytes` (a minimal valid GGUF header). Reuse these when testing services.
- For quality benchmarks it adds `ScriptedLlama` (answers computed per request; records `chat_requests` and `prompts`), `FakeDatasetStore` (`put(file, rows)` writes JSONL fixtures), `FakeCodeRunner` and `fake_quality_deps()`. `FakeSession::with_api(...)` wraps any `LlamaApi`, and `.context()` builds a `BenchmarkContext`.
- It is compiled for core's own tests and behind core's `testing` feature (`#[cfg(any(test, feature = "testing"))]`, likewise `ActivityGuardFactory`). `src-tauri` enables the feature only in `[dev-dependencies]`, so release builds never include the fakes.

Other test setups:
- **Command tests** (`src-tauri/src/commands/tests.rs`) call the plain command fns on an `AppState` built from these fakes and assert on the serialized JSON and error codes.
- **IPC tests** (`src-tauri/src/commands/ipc/tests.rs`) drive the real `ipc::handler()` on Tauri's mock runtime (`tauri` `test` feature, dev-only) to pin the wire contract: command names, argument keys (`request`, `filter`, `id`) and result/error JSON.
- **Wiring tests** (`src-tauri/src/tests.rs`) run `build_state` against a temp SQLite file with the real monitors.
- **`LlamaHttpClient` tests** run against an axum stand-in server on a random port (axum is a core dev-dependency only).
- **Launcher tests** use fake `llama-server` shell scripts.

## Environment notes

- Configuration comes from the process environment of the app (see `.env.example`; nothing loads `.env` automatically any more).
  - Keys: `LLAMA_SERVER_BIN`, `MODELS_DIR`, `IDLE_TIMEOUT_SECS` (0 disables the watchdog), `LLAMA_LOAD_TIMEOUT_SECS` and `RUST_LOG` (default `weight_check_lib=info,weight_check_core=info`).
- Tauri on Linux needs webkit2gtk-4.1, gtk3 and libsoup3 (installed on the developer's machine).
- The developer's machine often runs its own llama-server on port 8080, holding most of the 24 GB VRAM on the RX 7900 XTX (Vulkan build). Don't kill those processes. When testing, use a small model with partial offload (`n_gpu_layers`), or ask first. Quitting the app stops only the llama-server it spawned.

## Frontend (`ui/`)

Vite + React 19 single-page app, TypeScript, TanStack Router (file-based), TanStack Query, Tailwind CSS v4 (`@tailwindcss/vite`), Vitest + Testing Library. It builds to a static bundle in `ui/dist/` (the Tauri desktop shell will serve it). **Use pnpm, not npm.** Run commands from `ui/`:

```sh
pnpm tauri dev                                # desktop app; starts `pnpm dev` (http://localhost:5173, strictPort) for the webview
pnpm lint && pnpm typecheck && pnpm test && pnpm build
```

- Routes live in `src/routes/` (`__root.tsx`, `index.tsx`, `models.tsx`, `benchmarks.tsx`, `runs/index.tsx`, `runs/$id.tsx`). The router plugin in `vite.config.ts` regenerates `src/routeTree.gen.ts` on `dev`/`build`; it is committed so `tsc` works on a clean checkout. Commit it whenever routes change.
  - Page titles come from each route's `staticData.title` (`components/layout/DocumentTitle.tsx`).
  - `/runs` filters live in the URL: `validateSearch` is `validateRunSearch` from `components/runs/runFilterParams.ts`; read with `Route.useSearch()`, update with `useNavigate({ from: "/runs/" })`.
  - Use `Link`/`useNavigate` from `@tanstack/react-router` with typed `to`/`params`/`search`, never raw URL strings.
- Routes unmount when you navigate away, so form and mutation state resets on its own; in-flight mutations still finish and invalidate their queries.
- The UI talks to the Rust core through Tauri commands, never HTTP. `call<T>(command, args)` in `lib/api/client.ts` wraps `invoke` from `@tauri-apps/api/core`:
  - Command names are snake_case; argument keys are camelCase (e.g. `load_model({ request })`, `list_runs({ filter })`, `get_run({ id })`). Each command resolves with the same JSON the types in `types.ts` describe.
  - A rejected `{ code, message }` becomes `ApiError(code, message)`; a string rejection (Tauri argument deserialization) becomes `validation`; anything else `unknown`. Outside Tauri (`isTauri()` false, e.g. a plain browser tab) every call throws a `network` ApiError pointing at `pnpm tauri dev`.
  - `vite.config.ts` is set up for Tauri (`clearScreen: false`, `envPrefix` includes `TAURI_ENV_*`, webview `build.target`). The `src-tauri/` shell lives outside `ui/`.
- Layers mirror the backend:
  - `lib/api/` holds the command client, one function per command and the TS mirrors of the Rust types. Update `types.ts` when Rust DTOs change.
  - `lib/queries/` holds the TanStack Query hooks (plain `useQuery`/`useMutation`).
  - `components/ui/` holds the primitives, and `components/<feature>/` the feature components.
  - Route files in `src/routes/` only compose components.
- Component files export only components (react-refresh lint rule); put shared helpers in a `.ts` module (e.g. `components/ui/buttonClasses.ts`).
- Put form and parsing logic in pure modules (e.g. `components/models/loadForm.ts`, `components/benchmarks/configJson.ts`) and unit-test them.
- Tests are `*.test.ts(x)` files next to their source. `src/test/render.tsx` provides `renderWithQuery`, `renderWithRouter` (async; memory router, for components using `Link`/`useNavigate`, returns `router` to assert on `router.state.location`) and `mockInvoke({ command: (args) => result })`. `mockInvoke` builds on `mockIPC` from `@tauri-apps/api/mocks`, makes `isTauri()` true and returns a spy whose calls are `[command, args]`; throw or reject `{ code, message }` from a handler to simulate a command error. `src/test/setup.ts` clears the mocks after each test.
- Follow the Vercel Web Interface Guidelines (`/web-design-guidelines`):
  - labelled inputs, visible `:focus-visible` rings
  - `…` in loading text, `tabular-nums` for numbers, `Intl` formatting
  - Title Case for headings and buttons, error messages that give a next step
  - no browser `confirm()` dialogs
