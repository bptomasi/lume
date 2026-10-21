# Performance and qualification

Lume keeps expensive work away from navigation and rendering: bounded metadata
pages, replaceable read projections, independent catalog counts, ordered personal
writes, background content/image preparation and durable configuration
publication. Algorithm choice alone is not evidence of a speed improvement.

## Reproduce

`make check` runs correctness scenarios for every implemented benchmark. Full
measurements are separate:

```sh
make bench
make bench-configuration
make bench-code
make bench-interaction
make bench-images
make bench-notes
make bench-recommendations
```

The native benchmark also accepts focused flags, including
`--configuration-only`, `--welcome-only`, `--code-viewport-only`, `--images-only`,
`--tables-only` and `--recommendations-only`. Add `--stress` for larger workloads.
Setup precedes operation timers; resource reports identify process-wide
memory/I/O scope.
Routine reports go to ignored `.local/benchmarks/`. Promote reviewed evidence
here only when useful for a release or a specific comparison.

## Retained evidence

These reports retain their original binary hashes, dates and measurement scope.
They describe those revisions and environments, not a blanket guarantee for all
current hardware or a substitute for measuring later changes.

| Contract | Evidence |
| --- | --- |
| Long Unicode/code rows | [Checked frame comparison](performance/code-viewport.json) |
| Large-library navigation | [Repeated measurements](performance/navigation.json), [samples](performance/navigation-samples.csv), [Foot qualification](performance/navigation-foot.json) |
| Notebook search/context | [Representative and stress workloads](performance/notebook.json) |
| Native image interaction | [Preparation/scroll evidence](performance/images.json), [real Foot](performance/images-foot.json) |
| Declarative configuration | [Publication and verified rereads](performance/configuration.json), [real Foot interaction](performance/configuration-terminal.json) |
| Full-history feeds | [Large archive decode and content checks](performance/archive-feeds.json) |
| Reader tables and code backgrounds | [Unicode layout, cached frames and inspection](performance/tables.json) |
| Native welcome frame | [Representative/stress frames and interaction qualification](performance/welcome.json) |

Configuration measurements cover actual source/folder edits, stable portable
identities, TOML publication, comment retention and verified rereads. Three
release processes measured median iteration means of 14.660 ms at 250 sources
and 275.152 ms at 5,000 sources on the recorded Core Ultra 7 environment.
Publication runs on its own lane; these are not frame latency measurements or
comparisons against an earlier implementation. The source workloads and resource runner remain executable even
when a previous raw report is no longer retained.

The archive workload uses full-content RSS documents of 5,253,056 bytes with
200 entries and 13,365,856 bytes with 2,000 entries. Three release processes per
scenario, each with 40 iterations, measured median iteration means of 73.473 ms
and 192.036 ms respectively. Every article's identity, title and complete body
is checked. These describe bounded background parsing, not UI latency or a
speedup over the former 4 MiB rejection. Repeat with
`cargo bench --locked --bench reader -- --archive-feeds-only`, adding `--stress`
for the larger input; resource measurements include setup.

The table workload checks Unicode alignment and exact source correspondence at
64 rows/four columns and 1,000 rows/12 columns. Three release processes per
scenario measured median iteration means of 0.375/10.826 ms for preparation,
0.511/0.753 ms for cursor movement plus a selected cached frame, and
0.021/0.738 ms for opening and closing the cached inspector. Each process runs
40 layout/inspector iterations and 1,000 frames, including a fenced code block
with short and blank rows. Process RSS, including grammar and fixture setup,
ranges from 20,976–21,492 KiB and 42,892–44,216 KiB respectively. These measure
the implemented behavior without a speedup baseline. Repeat with
`cargo bench --locked --bench reader -- --tables-only`, adding `--stress` for
the larger input.

## Boundaries

TestBackend timings measure checked application work, not terminal transport or
physical display latency. The real Foot latency observer measures injected input
through a correlated synchronized frame and terminal DSR acknowledgement. It
excludes the compositor and physical key-to-photon time. At least 100 samples
per workload are required for its reported p99; compare repeated independent runs
before claiming an improvement.

Actual Foot and Alacritty qualification uses owned windows and temporary libraries,
checks cleanup and never changes terminal preferences or the host clipboard.
Desktop-dependent checks remain separate from ordinary local verification and
must be reported as unavailable if their environment cannot be provided.
See [development](development.md).

## Configuration and installer review

The [repeated before/after workloads](performance/review.json) compare the same
pinned toolchain and machine, with exact result, identity and comment checks.
At 5,000 subscriptions, median publication-plus-reread time fell from 269.069
ms to 187.792 ms (30% less). The actual revision-query SQL and row mapping fell
from 163.680 microseconds to 1.535 microseconds per check. At 250 subscriptions,
publication fell from 14.712 ms to 11.088 ms; revision checks fell from 9.400 to
1.705 microseconds. These are scoped operations, not whole-application speedups.

The revision query excludes the serialized snapshot and reuses its statement.
Single-writer merges avoid an intermediate JSON tree, projection reuses validated
definitions, and document patching reuses the validated input. Configuration
publication remains outside the input/render lane. Process RSS includes fixtures;
benchmarks are not physical key-to-photon measurements.

The same report qualifies exact-byte executable upgrades at 16/64 MiB and SQLite
snapshot migration at 1,000/10,000 articles. Upgrades verify that library and
configuration bytes remain unchanged. These new installer workloads have no
speedup baseline.

The [table-layout measurements](performance/configuration-format.json) record
current publication-plus-reread medians of 12.517 ms at 250 sources and
213.455 ms at 5,000 sources. Rewriting compact input, formatting atomically and
verifying a reread takes 7.064 ms and 168.330 ms respectively. Idle revision
checks take 1.525 and 1.550 microseconds. Each scenario uses three release
processes with 40 iterations; process RSS, including setup and all workloads,
ranges from 17,148–18,120 KiB and 83,096–88,716 KiB. These characterize the new
document representation, including its publication cost; they do not establish
a speedup. Explicit formatting and publication remain outside input/render work.

Real Foot qualification of the reviewed application used 50,002 articles,
16 sources and 64 smart lists, with 100 correlated samples per scenario. The
input-to-terminal-acknowledgement p99 was 5.464 ms during collection, 6.141 ms
with cached images and 22.174 ms for article switching. This excludes physical
presentation; it is not a before/after terminal speedup comparison. Native image
scroll, selection, welcome entry and terminal cleanup also passed. Alacritty was
not rerun for this revision because xdotool was unavailable.
