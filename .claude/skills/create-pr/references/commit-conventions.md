# Commit, branch and PR-title conventions

This repo uses [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/). PRs are squash-merged, so the PR title becomes the commit subject on `main` and follows the same rules.

## Format

```
type(scope)!: subject

Body: what changed and why, wrapped at 72 columns.

BREAKING CHANGE: what breaks and how to migrate.
<attribution trailer from the session's guidance, if any>
```

- `(scope)` is optional but preferred. `!` and the `BREAKING CHANGE:` footer appear only for breaking changes.
- One blank line between subject, body and footers. The footers (`BREAKING CHANGE:`, the attribution trailer) form the last paragraph.

## Types

| Type | Use for |
|---|---|
| `feat` | New user-visible capability (endpoint, benchmark, metric, monitor) |
| `fix` | Bug fix |
| `refactor` | Code change with no behaviour change |
| `perf` | Performance improvement |
| `test` | Tests only (including `service/testing.rs` fakes) |
| `docs` | Documentation only (`README.md`, `CLAUDE.md`, doc comments) |
| `ci` | `.github/workflows/**` |
| `build` | `backend/Cargo.toml`, `backend/Cargo.lock`, `backend/.cargo/config.toml`, `docker-compose.yml` |
| `chore` | Tooling and repo upkeep: `.claude/**`, `.gitignore`, `.env.example`, `.github/` templates |

A new migration is part of the feature or fix that needs it, not a separate commit type.

## Scopes

Resource / feature scopes:
- `models`: model discovery and lifecycle (`service/model_service.rs`, `presentation/models.rs`, load/unload, idle watchdog).
- `benchmarks`: the `Benchmark` trait, registry and individual benchmarks (`service/benchmarks/`, `presentation/benchmarks.rs`).
- `runs`: run execution and history (`service/benchmark_service.rs`, `presentation/runs.rs`).
- `system`: hardware/system info (`service/system_service.rs`, `presentation/system.rs`).

Layer / adapter scopes:
- `api`: cross-cutting `presentation/` changes (router, `dto.rs`, `extract.rs`, `error.rs`).
- `db`: `repository/` and `migrations/`.
- `llama`: `infra/llama/` (launcher, `llama-server` process, HTTP client).
- `monitor`: `infra/monitor/` (GPU and host monitors).
- `gguf`: `infra/gguf.rs`.
- `config`: `config.rs`, `.env.example`, `main.rs` wiring.
- `domain`: `domain/`, `error.rs`.

Repo scopes: `tests`, `skills` (`.claude/skills/`), `docs`, `deps`.

Pick the narrowest scope that covers the change. Prefer a resource (`models`, `benchmarks`) over a layer (`api`, `db`) when the change is about one resource, even if it touches several layers. Omit the scope when nothing fits.

## Subject

- Imperative mood: "add", "fix", "remove", not "added" / "adds".
- Lowercase first word, no trailing period.
- ≤72 characters including `type(scope): `; aim for ≤50.
- Says what the change does, not how or which files.

## Body

- Explain **what** and **why**, not a line-by-line how. The diff shows the how.
- Wrap at 72 columns. Bullets (`- `) are fine for several points.
- Omit the body only for trivial changes where the subject says everything.

## Breaking changes

Anything a client or deployer must react to: changed HTTP status codes or response shapes, removed or renamed endpoints or fields, renamed `.env` keys, metric names or units changed in the `metrics` table, migrations that drop or rename columns.

- Add `!` after the type/scope: `feat(runs)!: report throughput in tok/s`.
- Add a `BREAKING CHANGE:` footer describing the old → new behaviour and any migration.

## Granularity

- One logical change per commit. Its tests, migration and docs go in the **same** commit.
- Split when the staged diff mixes unrelated concerns (e.g. a bug fix plus a dependency bump). Don't split a feature from its own tests.

## Branch names

`<type>/<kebab-slug>`, slug 2-5 words, same type as the main commit.

Good: `feat/prompt-processing-bench`, `fix/watchdog-busy-unload`, `chore/create-pr-skill`.
Bad: `jc-changes`, `feature/Add_Bench`, `fix/fix-the-bug-where-unload-returns-500-while-busy`.

## PR title

Same rules as a commit subject.

- One commit: use its subject.
- Several commits: write one subject covering all of them. Type precedence: `feat` > `fix` > the most significant remaining type. Use a shared scope if there is one, else omit it. Add `!` if any commit is breaking.

## Examples

Good:

```
feat(benchmarks): add prompt-processing throughput benchmark
```

```
fix(models)!: return 409 when unloading a busy model

DELETE /models/active returned 500 while a benchmark held an
ActivityGuard. AppError::Busy now maps to 409 Conflict, so clients
can retry after the run finishes instead of treating it as a crash.

BREAKING CHANGE: unloading a model that is in use now returns 409
instead of 500.
```

```
chore(skills): adapt create-pr skill to the Rust backend
```

```
build(deps): bump axum to 0.8
```

Bad:

| Message | Problem |
|---|---|
| `Add bench, fix watchdog, bump deps` | No type; three unrelated changes in one subject |
| `fix: Fixed bug.` | Past tense, capitalised, period, says nothing |
| `feat(api): update dto.rs and runs.rs` | Lists files instead of the change |
| `chore: add decode benchmark` | Wrong type: a new benchmark is `feat` |
| `fix(config)!: rename IDLE_TIMEOUT_SECS` (no footer) | `!` without a `BREAKING CHANGE:` footer explaining the migration |
