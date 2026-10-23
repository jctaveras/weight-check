---
name: create-pr
description: Branch, stage, check (fmt, clippy, cargo test), commit (Conventional Commits), push, and open a draft GitHub PR for weight-check whose body is filled from the repo's PR template.
disable-model-invocation: true
argument-hint: "[ready] [base:<branch>] [skip-tests] [hint]"
allowed-tools: Bash(git status *) Bash(git diff *) Bash(git log *) Bash(git show *) Bash(git branch *) Bash(git rev-parse *) Bash(git rev-list *) Bash(git symbolic-ref *) Bash(git remote get-url *) Bash(git ls-remote *) Bash(git ls-files *) Bash(git check-ignore *) Bash(git cat-file -s *) Bash(git fetch *) Bash(git switch *) Bash(git add *) Bash(git restore --staged *) Bash(git commit *) Bash(git push *) Bash(cargo fmt *) Bash(cargo clippy *) Bash(cargo test *) Bash(~/.cargo/bin/cargo *) Bash(command -v cargo) Bash(printenv TEST_DATABASE_URL) Bash(gh auth status *) Bash(gh pr view *) Bash(gh pr create *) Read Grep Glob
---

# /create-pr

Ship the working-tree changes as a PR: branch → stage → check → commit → push → open the PR.

References (read when you reach the step that needs them):
- [references/commit-conventions.md](references/commit-conventions.md): commit messages, branch names, PR titles.
- [references/pr-description.md](references/pr-description.md): how to fill the PR body.

## Safety rules (apply at every step)

- Never force-push (`--force`, `--force-with-lease`, `+refspec`). Never rebase, reset or amend a commit that has been pushed.
- Never commit on the base branch.
- Never bypass hooks (`--no-verify`) unless the user explicitly asked for it in this conversation.
- Never stage secrets. Never use `git add -A`, `git add .` or `git add -u`; stage by explicit path.
- Never run `gh pr edit`, `gh pr merge`, `git reset` or anything outside these steps.
- Never kill `llama-server` processes or start the backend; the checks below need neither.
- When anything is ambiguous (what belongs in the change, the commit split, a check failure), stop and ask.

## Steps

### 0. Parse arguments

`$ARGUMENTS` is space-separated, any order:
- `ready`: open the PR ready for review (default: draft).
- `base:<branch>`: base branch.
- `skip-tests`: skip step 4.
- Anything else is a free-text hint about the change's intent; use it for the branch name, commit message and PR description.

### 1. Preflight

1. Run `git status`. Stop if it reports a merge, rebase, cherry-pick or revert in progress, unmerged paths, or `HEAD detached`.
2. Base branch: the `base:` argument; else `git symbolic-ref --short refs/remotes/origin/HEAD` minus `origin/`; else `main`.
3. `git fetch origin <base>`. If it fails because the remote has no `<base>` branch (`git ls-remote --heads origin <base>` is empty), stop: a PR needs a base on GitHub. Tell the user to push an initial commit to `<base>` first, and don't do it yourself.
4. `git status --porcelain -uall` for changes. Count unpushed commits: `git rev-list --count @{u}..HEAD` if the branch has an upstream, else `git rev-list --count origin/<base>..HEAD`.
5. No changes and no unpushed commits: say so and stop.
6. On a feature branch, run `gh pr view <branch> --json url,state,isDraft`. If it shows an `OPEN` PR: do steps 3-6 only (stage, check, commit, push), then report that PR's URL. Don't create another.

### 2. Branch

- **On the base branch:** derive `<type>/<kebab-slug>` from the diff and hint (see commit-conventions.md). If `git branch -a --list '*<name>'` shows it taken, append `-2`, `-3`, … Run `git switch -c <name>`.
  - If the base branch had unpushed local commits, they move with the new branch, but the local base still points at them. Tell the user; don't reset it.
- **On another branch:** reuse it. Warn (don't rename) if it doesn't match `<type>/<kebab-slug>`.

### 3. Stage

1. From `git status --porcelain -uall`, pick the paths that belong to the change and `git add -- <path> …`.
2. Never stage:
   - `.env` / `.env.*` (except `.env.example`), keys and credentials (`*.pem`, `*.key`, `id_*`, `*.p12`, `credentials*`, `secrets*`);
   - build and coverage output: `target/` (including `backend/target/`), `*.profraw`, `*.profdata`, `lcov.info`;
   - model files (`*.gguf`, `*.safetensors`, `*.bin`);
   - `plan.md`.
3. Ask first before staging `.claude/settings.local.json`, or any untracked file that isn't clearly part of the change.
4. Check what got staged:
   - `git diff --cached --numstat`: a `-	-` row is a binary. Unstage it (`git restore --staged -- <path>`) and ask.
   - `git cat-file -s :<path>` for new or large files: unstage anything over 1 MB (1048576 bytes) and ask. `backend/Cargo.lock` is the exception: stage it whenever `Cargo.toml` dependencies changed.
5. Secret scan:
   ```
   git diff --cached --name-only -G'sk-ant-[A-Za-z0-9_-]{20,}|AKIA[0-9A-Z]{16}|-----BEGIN [A-Z ]*PRIVATE KEY-----|gh[pousr]_[A-Za-z0-9]{36}|github_pat_[A-Za-z0-9_]{20,}|hf_[A-Za-z0-9]{30,}'
   ```
   Any file listed: `git restore --staged -- <file>`, show the user the file and pattern (not the secret), and stop.
6. Show `git diff --cached --stat`. If the change mixes unrelated concerns, propose a split into several commits (paths per commit) and ask. When splitting, stage and commit one group at a time (steps 3 and 5 per group; checks once).

### 4. Check

- Skip if `skip-tests` was passed, or the change is docs-only (below). Record "skipped (reason)".
- Cargo lives in `~/.cargo/bin` and may not be on PATH: if `command -v cargo` finds nothing, use `~/.cargo/bin/cargo` in place of `cargo`. Run from the repo root with `--manifest-path`:
  1. `cargo fmt --manifest-path backend/Cargo.toml --check`
  2. `cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings`
  3. `cargo test --manifest-path backend/Cargo.toml`
- Record each command's result; for `cargo test`, the exact `test result:` lines (one per test binary) and the total passed/failed/ignored.
- Postgres integration test (`backend/tests/postgres.rs`): it silently passes without `TEST_DATABASE_URL`. Check `printenv TEST_DATABASE_URL`; if it's empty, record "Postgres integration test not exercised (TEST_DATABASE_URL unset)". Never report it as passing in that case. Don't start Postgres yourself.
- On a `fmt` failure: show the diff and ask whether to run `cargo fmt --manifest-path backend/Cargo.toml` and restage the touched files. On a clippy or test failure: show it and ask whether to continue or stop. Never report a failure or a skip as a pass.

Docs-only rule: every changed file matches `*.md`, `docs/*`, `LICENSE*`, `*.png`, `*.jpg`, `*.jpeg`, `*.gif` or `*.svg`, **and** none is under `.claude/` or is `CLAUDE.md` (those count as code).

### 5. Commit

1. Write the message per [commit-conventions.md](references/commit-conventions.md).
2. End it with the commit attribution trailer from **this session's** attribution guidance (the system reminder about git commits), copied verbatim. Never type a model name yourself. If the session gives no commit attribution, add none.
3. Commit with a heredoc:
   ```
   git commit -F - <<'EOF'
   type(scope): subject

   Body.

   <trailer from the session's attribution guidance>
   EOF
   ```
4. If a commit hook fails, fix the cause and make a **new** commit. Don't amend.
5. Show `git log -1 --format=%B`.

### 6. Push

Run `git push -u origin <branch>` and act on the result:

| Output contains | Action |
|---|---|
| Push succeeded | Continue. |
| A pre-push hook printed an error and the push exited non-zero | Show the hook output. **Stop. Open no PR.** Don't offer `--no-verify` unless asked. |
| `! [rejected]` with `(non-fast-forward)` or `(fetch first)` | Remote has commits you don't. **Stop.** Explain; let the user integrate. Don't rebase, pull or force. |
| Any other failure (network, auth, …) | Stop and show the error. |

### 7. Describe

1. Gather facts: `git log --format='%h %s%n%b' origin/<base>..HEAD`, `git diff --stat origin/<base>...HEAD`, and the diffs of the files that matter.
2. Read the template `${CLAUDE_PROJECT_DIR}/.github/pull_request_template.md`. If it's missing, use the headings `## Summary`, `### ⚠️ Behaviour change`, `## Changes`, `## Test plan`.
3. Fill it per [pr-description.md](references/pr-description.md): strip every `<!-- … -->` comment, drop `### ⚠️ Behaviour change` when nothing externally observable changed, and use only facts from this run in the Test plan.
4. Append the PR attribution footer from **this session's** attribution guidance, verbatim, after a blank line. None given: no footer.
5. Title: the commit subject if there is one commit; otherwise one Conventional Commits subject covering them all (see commit-conventions.md). PRs are squash-merged, so the title becomes the commit on `<base>`.
6. Write the body to `~/.cache/create-pr/<repo>/<branch-with-/-as-->-<ts>.md` (`<repo>` = basename of `git rev-parse --show-toplevel`; `<ts>` from `git log -1 --format=%cd --date=format:%Y%m%d-%H%M%S`). Never write it inside the repo.

### 8. Open

- `gh auth status` succeeds:
  ```
  gh pr create --draft --base <base> --head <branch> --title "<title>" --body-file <body file>
  ```
  Omit `--draft` when `ready` was passed.
- Otherwise (no `gh`, or not logged in): don't install or log in. Print the body file path and the compare URL `https://github.com/<owner>/<repo>/compare/<base>...<branch>?expand=1`, with `<owner>/<repo>` parsed from `git remote get-url origin`:
  - SSH `git@github.com:<owner>/<repo>.git` or `ssh://git@github.com/<owner>/<repo>.git`;
  - HTTPS `https://github.com/<owner>/<repo>(.git)`.
  Tell the user to paste the body file and keep the PR a draft unless they passed `ready`.

### 9. Report

One short block:
- PR URL (or compare URL + body file path), and draft or ready;
- branch and base;
- commits (`<short sha> <subject>`);
- checks: fmt / clippy / test results (exact `cargo test` totals), or "skipped (reason)", or "failed, continued at user's request";
- Postgres integration test: exercised or not (TEST_DATABASE_URL unset);
- anything left for the user (e.g. local base branch still holds the moved commits).
