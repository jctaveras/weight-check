## Summary

<!-- 1-2 sentences: why this change exists, then what it does at the highest level. Add "Closes #<n>" if it resolves an issue. -->

### ⚠️ Behaviour change

<!-- Delete this section if nothing externally observable changes.
Otherwise state before → after (new value in bold) for: command names/arguments, error codes, response shapes, env keys, metric names/units, or the database schema.
List every new migration (core/migrations/NNNN_*.sql) and what it changes; migrations run automatically at startup. -->

## Changes

<!-- Group by area: a bold line per group, then bullets naming the file or symbol and what changed (and why, when not obvious).
Most important group first; docs and housekeeping last. -->

## Test plan

<!-- [x] only for what you actually ran, with the result. Leave anything not run as [ ] and say why. -->

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `pnpm lint && pnpm typecheck && pnpm test && pnpm build` (in `ui/`)
