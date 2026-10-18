# Lume development contract

Read `/home/bpt/.codex/RTK.md` when available. This repository implements an
independent RSS/Atom/JSON Feed terminal reader named Lume.

- Write readable English code from the first edit. Prefer 80 columns, explicit
  fields/branches and cohesive functions around 30–40 lines. Format each slice.
- Keep dependencies explicit, contracts narrow and domain state out of adapters.
  Document intent, lifecycle and failures at their owning module.
- Follow `docs/dependency-security.md` before adding/updating dependencies.
- Forbid unsafe in our code. No production unwrap, expect, panic, todo or dbg.
- Keep production and test SQL in its owner's `sql/`. Only test targets may
  include `test_*.sql`. Preserve user data across publisher refreshes.
- Root `make check` aggregates local checks. `make check-release` also requires
  real HTTP qualification; report failures without silently skipping them.
- Add native behavioral/failure tests and correctness-checked representative
  benchmarks for implemented capabilities. Never infer speed from design alone.
- Review readability and ownership after tests. Do not mechanically split
  functions to satisfy Clippy or add blanket lint exceptions.
- Use the exact Rust version in `rust-toolchain.toml` for local/CI release gates.
  Explicitly identify any environment-only toolchain/source substitution.
- Do not start containers: the product requires no server or external database.
