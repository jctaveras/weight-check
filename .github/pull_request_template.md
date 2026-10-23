## Summary

<!-- 1-2 sentences: why this change exists, then what it does at the highest level. Add "Closes #<n>" if it resolves an issue. -->

### ⚠️ Behaviour change

<!-- Delete this section if nothing externally observable changes.
Otherwise state before → after (new value in bold) for: HTTP status codes, response shapes, endpoints, .env keys, metric names/units, or the database schema.
List every new migration (backend/migrations/NNNN_*.sql) and what it changes; migrations run automatically at startup. -->

## Changes

<!-- Group by area: a bold line per group, then bullets naming the file or symbol and what changed (and why, when not obvious).
Most important group first; docs and housekeeping last. -->

## Test plan

<!-- [x] only for what you actually ran, with the result. Leave anything not run as [ ] and say why. -->

- [ ] `cargo fmt --check`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test`
- [ ] Postgres integration test (`TEST_DATABASE_URL=… cargo test --test postgres`)
