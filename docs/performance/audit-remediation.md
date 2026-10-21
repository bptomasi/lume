# Durability and navigation qualification — 2026-10-21

Environment: Linux x86_64, Intel Core Ultra 7 155H (22 logical CPUs), exact
Rust 1.99.1, locked release profile with thin LTO. Build artifacts were kept
outside the checkout. Setup is excluded from operation timers unless named;
results are local measurements, not hardware-independent latency guarantees.

## Dense annotations

Three repeated release runs, each with 40 correctness-checked cycles, projected
3,000 known quotations through 30,000 source mappings. Each indexed cycle
includes construction of the source interval index and all quote queries.
Independent generated tests also cover reordered/overlapping tokens and
indivisible normalized entities.

| Run | Linear reference, ms/cycle | Indexed, ms/cycle |
| --- | ---: | ---: |
| 1 | 94.774 | 0.513 |
| 2 | 99.194 | 0.525 |
| 3 | 96.036 | 0.519 |

The reference and indexed variants produce the same expected byte ranges.
Process peak RSS in the two timed repeats was 6,620 and 6,392 KiB; this is
whole-process RSS, not the isolated index allocation. The comparison measures
this projection operation only. It is not a claim that every interaction or
whole-app startup became proportionally faster.

A separate revised-source workload retains 1,800 annotations over a 196,085-byte
publisher document: one third relocate to a unique quotation and two thirds
remain explicitly unplaced because their quotations are ambiguous or missing.
All 40 cycles verify these categories, note counts and source revision. Full
synchronous App construction plus an 80-column reflow averaged 53.569 ms;
this includes opening SQLite, reading metadata/notes and layout. Interactive
startup uses the asynchronous path instead of this legacy synchronous path.

## Startup, navigation and recovery

The stress library contains 4,000 articles, 40 subscriptions and 200 saved
queries. Over 40 release cycles:

- Consistent online snapshot plus validated isolated restore: 168.625 ms/cycle.
- First prepared reader, including opening the existing library, metadata and
  worker startup: 33.546 ms mean.
- Jump admission on the input thread: 21.823 microseconds mean. Delivery is
  verified separately against the captured article identity.
- Complete startup/query-count refresh, jump delivery and session durability:
  480.680 ms/cycle. This includes waiting for all 200 query counts; it is not
  input-thread blocking or first-frame latency.

An earlier equivalent timed stress run used 31,684 KiB peak process RSS.
Snapshots include committed WAL data, preserve configuration and annotations,
and restore into new locations. Setup, filesystem cache, background scheduling
and machine power policy affect these numbers.

Real Foot qualification additionally observed the first lighthouse bytes
26.152 ms after process launch on an existing V1 fixture requiring migration.
The library/article milestones were approximately 261 ms, including the
qualification driver's deliberate 250 ms pause before pressing the entry key.
These measurements observe PTY output; they do not measure compositor or physical
display latency. Initial configuration adoption/migration precedes disposable
article projections and is not omitted from the process measurement.

## Reproduction

```sh
cargo bench --locked --bench reader -- --annotation-projection-only --stress
cargo bench --locked --bench reader -- --dense-annotations-only --stress
cargo bench --locked --bench reader -- --recovery-navigation-only --stress
make check-foot
```

For whole-process resource evidence, run the built benchmark executable directly
under `/usr/bin/time -v`; timing `cargo bench` can include compilation. Native
Foot reports record the executable hash, source version, cell geometry, output
milestones, independently decoded Sixel pixels and terminal cleanup.
