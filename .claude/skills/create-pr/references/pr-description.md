# Filling the PR description

Read the template at `${CLAUDE_PROJECT_DIR}/.github/pull_request_template.md` and fill it. If the file is missing, use these headings in this order:

```
## Summary
### ⚠️ Behaviour change
## Changes
## Test plan
```

## General

- Remove every `<!-- … -->` guidance comment. No placeholder text may survive.
- Facts only. Everything must be traceable to the diff, the commit messages, or output you saw in this run. No guesses about CI, reviewers or benchmark numbers you didn't measure.
- Write for a reviewer who hasn't seen the conversation: name files and symbols in backticks (`service/model_service.rs::ModelSession::checkout`).
- Plain, short sentences. No marketing ("robust", "seamless"), no emojis besides the template's ⚠️.

## Summary

- 1-2 sentences on **why** the change exists, then what it does at the highest level.
- Add `Closes #<n>` only if the user or a commit names the issue. Never invent an issue number.

Example: "Unloading a model while a benchmark was running returned a 500. `AppError::Busy` now maps to 409, and the idle watchdog test covers the busy case."

## ⚠️ Behaviour change

- Keep this section only if something **externally observable** changes: HTTP status codes, response shapes, endpoints, `.env` keys, metric names/units in the `metrics` table, or the database schema. Otherwise delete the heading entirely.
- State before → after, with the **new** value in bold, plus why or what it now matches.
- Any commit with `!` / `BREAKING CHANGE:` must appear here.
- A new migration always gets a line here naming the file (`backend/migrations/0002_….sql`) and what it changes, since it runs automatically at startup.

Example: "`DELETE /models/active` while a run holds the model now returns **409 Conflict** instead of 500, matching the other `Busy` responses."

## Changes

- Group by area. One bold line per group (`**Busy-model handling**`), then bullets.
- Most important group first; docs and housekeeping last.
- Each bullet names the file or symbol and says what changed and, when not obvious, why ("so the watchdog can't unload a model mid-run").
- Respect the layering in the description too: say which layer a change lives in when it matters (`presentation/error.rs` mapping vs `service/` logic).
- Summarise; don't dump the diff or list every touched file. Tests go with the group they cover, unless tests are the change.

## Test plan

- Keep the template's four default items (fmt, clippy, test, Postgres). Tick each one only if it ran in step 4, adding its result; otherwise leave it `[ ]` with the reason.
- `- [x]` only for something this run actually ran or observed, with the exact command and its result, e.g.:
  - `` - [x] `cargo fmt --check` `` 
  - `` - [x] `cargo clippy --all-targets -- -D warnings` ``
  - `` - [x] `cargo test`: 84 passed, 0 failed, 0 ignored ``
- Postgres integration test:
  - Ran with `TEST_DATABASE_URL` set: `` - [x] `cargo test --test postgres` against `TEST_DATABASE_URL` ``.
  - Not set: `- [ ] Postgres integration test (TEST_DATABASE_URL unset; the test skipped)`. Never `[x]`.
- `- [ ]` for everything not run here: manual runs against a real `llama-server`, CI jobs, benchmark numbers on real hardware. Say why if useful.
- Checks skipped (`skip-tests` or docs-only): write `- [ ] Checks not run (<reason>)`. Never `[x]`.
- A check failed and the user chose to continue: `- [ ]` with the failing summary line.

## Footer

The template has no footer. Append the PR attribution line from this session's attribution guidance, verbatim, after a blank line. If the session gives none, add nothing.
