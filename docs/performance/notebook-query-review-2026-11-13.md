# Notebook query follow-up — 2026-11-13

## Scope

This follows the measured notebook regression in the
[architecture review](architecture-review-2026-11-13.md). It keeps the patched
SQLite 3.54.1/rusqlite 0.41.0 dependencies, schema, bounded caches and source
validation. No user database or subscription was used for experiments.

The primary comparison uses the unchanged `--notebook-only` workload
against the preserved full-feature `fe81ac6` executable. The older SQLite 3.46
baseline is a separate combined comparison. It cannot isolate an engine update.
The previous prepared-statement cache prototype remains rejected and reverted.

## Investigation

Opening/migration checks execute before the operation timers. The original
Store notebook SQL and export implementation also predate the ownership work.
The slow direct Store/export operations therefore do not execute App state
transitions. This does not isolate whole-binary optimization or system effects.

Two additional experiments tested the database hypothesis:

1. Preserved old/current bundled SQLite debug archives were linked into
   temporary shared libraries. Raw API calls used one synthetic database,
   separating preparation, execution and release while collecting statement
   counters. This is diagnostic evidence, not production release timing.
2. Identical standalone Rust probes used actual Rust 1.99.1, thin LTO, one
   codegen unit and each original/patched bundled library pair. They compiled
   outside the repository and read only the same synthetic 10,000-note,
   100-article fixture. Product manifests and locks were unchanged. Twelve
   processes per journal-mode control followed ABBA ordering over three blocks.
   Each query/process had
   one excluded 5,000-iteration warmup sample and five measured samples.

The standalone probes did **not** reproduce the large whole-program regression.
For the original note-summary query, the median of 30 sample means was:

| Stage, microseconds | Old libraries | Patched libraries |
| --- | ---: | ---: |
| Prepare | 16.8610 | 16.7176 |
| Bind and execute to first row | 4.4933 | 4.3770 |
| Materialize owned values | 0.5949 | 0.5983 |
| Reset | 0.6691 | 0.6106 |
| Finalize | 0.7645 | 0.6430 |

That first control used the fixture's default rollback journal. Repeating
with persistent WAL, as used by the product, also failed to reproduce the
regression: note-summary preparation was 15.4091 versus 15.0349 µs, and first-row
execution 2.0032 versus 1.8775 µs. The binaries and logical fixture stayed the
same. Article and metadata probes also had similar per-stage timings in both
journal modes. These probes
use generic owned SQLite values, not the application's complete domain mappers,
JSON decoding, multi-query snapshot workflow or file publication. They narrow the hypothesis; they
do not prove the engine is unrelated to every workload. In particular, the
filtered FTS workload was not isolated by those point-query probes. The exact
historical cause of the combined regression remains unresolved. It is not
justified to attribute the measured slowdown to SQLite alone.

## Removed work

The selected note context previously loaded a complete article presentation
record, including a count of all article comments, to obtain its URL. Feed
notes then issued a separate body query. A bounded source query now returns
the URL and the Feed body together. Full notes request no Feed body and retain
the existing independent-cache URL, nonempty-text and revision checks.

Note export previously loaded a complete note preview and complete article
presentation record, then discarded almost all those fields. It now reads
only note ownership and the citation URL. The joins retain active-note/feed/
metadata existence checks. The same annotation snapshot still supplies full
text, tags, links, cards, schedules and metadata; backlinks remain unchanged.
Missing/trashed notes retain the existing public context and underlying error.
Both operations keep their original read transaction boundary.

No result cache or new invalidation policy is involved. Prepared-statement
cache size, migrations, publisher data and personal history are unchanged.

## Reproducible query-stage profiling

A new checked workload uses the existing production SQL files and the same
notebook fixture. It compares narrow results with the corresponding reference
fields, verifies IDs, tags and article note counts, and checks every iteration.

```sh
cargo +1.99.1 bench --locked --bench reader -- --notebook-profile-only
cargo +1.99.1 bench --locked --profile dev --bench reader -- \
    --notebook-profile-only --test
```

It separates preparation, binding/execution to the first row, owned-value
materialization, reset and finalization. Counter collection, equality checks
and printing are outside the individual stage intervals. Timer overhead remains
in these microsecond measurements; the complete workflow benchmark determines
performance acceptance. The profiler runs each query without statement reuse.

SQLite VM-step counts measure virtual-machine work, not CPU instructions or
physical latency. See the primary documentation for
[statement status](https://sqlite.org/c3ref/stmt_status.html),
[counter meanings](https://sqlite.org/c3ref/c_stmtstatus_counter.html) and
[preparation](https://sqlite.org/c3ref/prepare.html).

The profiling fixture has 10,000 notes over 100 sources, so the original
article projection counts 100 notes on each read. Export workflow fixtures
have 100 notes over 100 sources. Stage counts must not be extrapolated into
an end-to-end speedup without the separate workflow measurements.

## Production-stage results

Five measured process means follow one excluded warmup process. Each query
has 4,000 correctness-checked iterations per process; all use the patched
engine in the actual optimized reader benchmark.

| Query | Prepare µs | Execute/bind µs | Materialize µs | Reset µs | Finalize µs | VM steps |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Original note preview | 15.085 | 1.677 | 0.548 | 0.547 | 0.600 | 64 |
| Original article metadata/count | 6.583 | 2.711 | 0.394 | 0.415 | 0.246 | 331 |
| Export ownership/URL | 6.187 | 0.761 | 0.102 | 0.417 | 0.087 | 24 |
| Feed source | 1.385 | 0.464 | 0.095 | 0.330 | 0.063 | 12 |
| Full source without Feed body | 1.400 | 0.459 | 0.079 | 0.332 | 0.061 | 11 |

The note context replaces the original article projection and separate Feed
body lookup with the source query. Export replaces both original projections
with the ownership/URL query. It does not drop annotation or backlink reads.

## Complete-workflow results

Both production comparisons use one excluded warmup per binary followed by
three sequential ABBA blocks: six observations per binary per workload.
All 28 processes, including four warmups across both comparisons, succeeded.
Each table reports the median of six harness run means, not a pooled latency
percentile. Normal checks cover 40 iterations, context and frame work 200;
initial export is one operation per process. Inputs, correctness assertions
and the existing timed workflow were unchanged. Only the fixture helper's
visibility changed to let the profiler reuse it.

The primary control uses the same patched engine and wrapper in both binaries:

| Checked workflow | `fe81ac6` ms | Candidate ms | Time change |
| --- | ---: | ---: | ---: |
| Filtered FTS/last page/snippets | 6.5600 | 6.4055 | −2.4% |
| Owned context/source/Markdown | 0.0800 | 0.0645 | −19.4% |
| Checkpoint/metadata/review/repair/undo | 3.0955 | 3.0085 | −2.8% |
| Nested wikilinks/backlink | 0.3355 | 0.3335 | −0.6% |
| Note key/ready context/actual frame | 0.2920 | 0.2650 | −9.2% |
| Initial files/citations/publication | 13.6250 | 8.9085 | −34.6% |
| Incremental files/exact-byte ownership | 10.0245 | 6.4580 | −35.6% |

The strongest gains are the changed context and export paths. Small changes
in untouched FTS/study/wikilink work are not claimed as targeted improvements.
The separate comparison with the original old-engine frozen baseline is:

| Checked workflow | Frozen baseline ms | Candidate ms | Time change |
| --- | ---: | ---: | ---: |
| Filtered FTS/last page/snippets | 5.8850 | 6.5645 | +11.5% |
| Owned context/source/Markdown | 0.0535 | 0.0660 | +23.4% |
| Checkpoint/metadata/review/repair/undo | 2.6245 | 3.0105 | +14.7% |
| Nested wikilinks/backlink | 0.2755 | 0.3325 | +20.7% |
| Note key/ready context/actual frame | 0.2560 | 0.2800 | +9.4% |
| Initial files/citations/publication | 9.9195 | 8.8900 | −10.4% |
| Incremental files/exact-byte ownership | 7.1035 | 6.4725 | −8.9% |

Export now beats that older baseline in these fixtures. The remaining notebook
regression is **partially recovered**, not eliminated. The original combined
comparison includes engine/wrapper/security and architecture changes; this
follow-up does not isolate the historical cause. Its point-query probes do
not replace a separate FTS or complete-workflow CPU profile.

No builds, test suites or agent jobs overlapped any timings. CPU frequency,
host scheduling and ordinary lightweight evidence I/O were uncontrolled.
Measurements exclude physical display/network latency. Whole-process RSS
includes fixture construction and is not App or query-cache heap usage.
Binary layout and differing host conditions also limit attribution; absolute
candidate medians differ slightly between the two paired comparisons.

## Validation and provenance

Actual Rust 1.99.1, unchanged locked dependencies, thin LTO, one release
codegen unit and debuginfo stripping were used. Incremental compilation was
disabled; Cargo jobs and test threads were capped at two. No toolchain/source
substitution, containers, dependency rollback or schema change was used.

- Root `make check`: 1,189 native cases passed, zero failed, 34 HTTP ignored;
  142 Python tooling tests, formatting, architecture/SQL/license checks and
  strict all-target Clippy passed. Checked benchmark scenarios also passed.
- The 62 affected native cases passed in both default and minimal builds,
  including three new source/annotation/recovery scenarios. Minimal strict
  affected Clippy also passed.
- The profiling timer's finalization endpoint was then tightened to exclude
  bookkeeping. Focused checked dev/release profiling and final all-target
  strict Clippy passed with that instrumentation-only adjustment.
- Existing optimized notebook correctness mode and all measured workflows
  retained their assertions. New tests cover complete Unicode Feed text,
  cross-connection body/tags/metadata/card edits, Full cache revision/URL
  invalidation, missing/Trash errors and recovery through restore/undo.

An initial profiler fixture function exceeded the configured 40-line limit;
reference loading and probe construction were separated by responsibility,
then the complete check passed. No check was weakened. These are commit
qualification results, not a new `make check-release`, HTTP or Foot run.

The candidate was built from `addb4d4` plus the eight recorded source changes;
its exact source hashes and artifact identity are in the
[summary JSON](data/notebook-2026-11-13-summary.json). The benchmark SHA-256 is
`f4a1cf7b193649485d5eaef815bc8a17190109db0bac1bc86db959146dceda42`.
The unchanged baseline hashes remain in the earlier architecture report.
The accepted source commit is `549724d0f4d140eea98d3e2f84d7836af52d6909`.
Every measured source hash was checked against that commit. Its Git revision
was not embedded in this already-built benchmark.

Versioned observations:

- [Same-engine workflows](data/notebook-2026-11-13-same-engine.csv).
- [Combined old-baseline workflows](data/notebook-2026-11-13-combined.csv).
- [Production query stages](data/notebook-2026-11-13-phases.csv).
- [Standalone rollback-journal point queries](data/notebook-2026-11-13-isolated-libraries.csv).
- [Standalone WAL point queries](data/notebook-2026-11-13-isolated-wal.csv).

Temporary logs, scripts, standalone crate locks/executables, synthetic database
and raw observations remain in `/tmp/lume-notebook-diagnosis`. They are not
permanent Git artifacts. The production profiler is versioned in the benchmark
suite; the old libraries exist only in the isolated diagnostic environment.
