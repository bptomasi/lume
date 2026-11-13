# Repeated notebook work — 2026-11-13

## Changes and contracts

This follows the [query diagnosis](notebook-query-review-2026-11-13.md).
Three read paths performed work whose results were duplicated or discarded:

| Path | Previous work | Reduced work |
| --- | --- | --- |
| Selected-note context | Fetch a preview with text/tags/metadata, fetch the complete annotation again, then fetch source context. | Load one source header and one complete annotation. Derive the bounded preview from that annotation. |
| Note-file enumeration | Search/count the entire active notebook on every page, decode previews, then retain only IDs. | Fetch at most 100 ordered IDs per page, without a count or preview projection. |
| Out-of-range notebook page | Run the complete search, discover an empty page, then repeat the count/search for the last page. | Count once, resolve the requested offset and read the selected page within that same snapshot. |

`note_detail` keeps its public result shape and read transaction. Tags, kind,
status, pin, timestamps and complete note/reference state remain live. Its
summary still omits reference edges. Source availability, Full cache URL and
revision checks, and complete Feed text remain authoritative. The preview
matches SQLite's 512-character `substr`, including combining characters and
NUL termination; a quote may contain NUL even though ordinary note text cannot.
Full quotes and private note bodies are never shortened in storage.

`note_export_ids` preserves the existing active-note joins and notebook order:
pin, update time and ID descending. It removes counts, unused preview text,
tag JSON encoding/decoding and empty-query parsing from export enumeration.
Publication, exact-byte ownership, local-edit conflict handling and per-note
snapshot checks are unchanged. OFFSET traversal is retained; this is not a
cursor implementation or a whole-export frozen snapshot. Concurrent edits can
still affect later pages, as with the previous export.

`search_notes_clamped` resolves an out-of-range request before fetching rows.
Count, resolved offset, preview rows and snippets share one SQLite snapshot.
Empty results resolve to offset zero. The existing `search_notes` exact-offset
API still returns an empty page for ordinary out-of-range offsets and rejects
unrepresentable offsets. Neither method retains result counts between calls.

No new result cache, invalidation policy, dependency, schema or configuration
change is involved. The patched SQLite remains in place.

## Measurement method

The primary binary reference is the immediately preceding accepted runtime,
`549724d` (documentation subsequently committed as `fc4ed3c`). Its preserved
benchmark hash is
`f4a1cf7b193649485d5eaef815bc8a17190109db0bac1bc86db959146dceda42`.
It was copied unchanged, not rebuilt. The new reader benchmark hash is
`bca1641c717274032aeefdee329f9feff88b68574c11917ef0e335edd6a9bdd9`.

Both use full defaults, Rust 1.99.1, thin LTO, one codegen unit and debuginfo
stripping. The actual compiler, environment, dirty-source identities and
artifact are recorded in the [summary](data/redundancy-2026-11-13-summary.json).
Cargo jobs and test threads were capped at two; incremental compilation was
disabled. No compiler/source substitution or containers were used.

The unchanged `--notebook-only` workflow ran one excluded warmup per binary,
then three ABBA blocks, producing six measured observations per binary. A
separate comparison used the original SQLite 3.46 baseline. Both comparisons
retained every correctness assertion and all observations: 28 successful
processes including four warmups. Values below are medians of six per-run
means, not pooled per-operation latency percentiles.

No builds, tests or agent jobs overlapped the timing runs. Host scheduling and
CPU frequency were uncontrolled. Small overlapping changes in unchanged work
are not interpreted as wins or attributed to an individual code change.
Timings exclude physical terminal/network latency. Whole-process RSS includes
fixture creation and is not a measurement of App or query-cache heap size.

## Complete-workflow comparison with the previous optimization

The fixture contains 10,000 notes over 100 sources; file export uses a separate
100-note/100-source fixture. Workflow counts and timed closures are unchanged.

| Checked operation | Previous ms | New ms | Time change |
| --- | ---: | ---: | ---: |
| Filtered FTS/last page/snippets | 6.5230 | 6.7080 | +2.8% |
| Owned note/source context/Markdown | 0.0655 | 0.0505 | −22.9% |
| Checkpoint/metadata/review/repair/undo | 3.0530 | 3.0875 | +1.1% |
| Nested wikilinks/backlink | 0.3255 | 0.3365 | +3.4% |
| Note key/ready context/actual frame | 0.2715 | 0.2495 | −8.1% |
| Initial files/citations/publication | 9.1090 | 8.5275 | −6.4% |
| Incremental files/exact-byte ownership | 6.6130 | 6.0815 | −8.0% |

The changed context and export paths improved. Normal filtered search, study
mutations and wikilink writes show small increases in this sample rather than
a demonstrated improvement. They were not the intended optimization targets
and must not be included in a claim that every operation became faster.

## Comparison with the original frozen baseline

This comparison also includes the earlier architecture/security/library
updates. It is a combined control, not an isolated SQLite experiment.
Candidate medians differ between paired groups because host conditions vary.

| Checked operation | Original ms | New ms | Time change |
| --- | ---: | ---: | ---: |
| Filtered FTS/last page/snippets | 6.0070 | 6.5855 | +9.6% |
| Owned note/source context/Markdown | 0.0550 | 0.0495 | −10.0% |
| Checkpoint/metadata/review/repair/undo | 2.6420 | 3.0835 | +16.7% |
| Nested wikilinks/backlink | 0.2735 | 0.3410 | +24.7% |
| Note key/ready context/actual frame | 0.2480 | 0.2520 | +1.6% |
| Initial files/citations/publication | 10.2780 | 8.3895 | −18.4% |
| Incremental files/exact-byte ownership | 7.0810 | 5.9670 | −15.7% |

Context and exports now beat this original baseline in these fixtures; the
frame comparison is approximately stable. FTS, study and wikilink regressions
remain. The exact historical cause of the combined slowdown is still not
isolated. These improvements do not justify attributing that cause to SQLite
or claiming a universal Lume speedup.

## Direct repeated-work comparison

The new checked `--notebook-redundancy-only` workload compares two APIs inside
the same optimized candidate. Export enumeration verifies every ID and order
against the preview-search reference. Stale-page recovery verifies the same
1,000 matches, last 100 IDs and FTS snippets as the repeated-search reference.

Six processes ran each mode; the first was an excluded warmup. Export
enumeration covers all 10,000 IDs, with four iterations per process; page
recovery covers 40 iterations. Reference work runs before reduced work in each
process. An untimed reference enumeration warms the dataset first, but the
API order is fixed. These are same-process reference comparisons, not the
old/new binary comparison above or a physical interactive latency claim.

| Operation | Reference ms | Reduced ms | Time change |
| --- | ---: | ---: | ---: |
| Enumerate 10,000 export IDs | 1,370.840 | 403.024 | −70.6% |
| Recover an out-of-range filtered page | 9.978 | 6.636 | −33.5% |

The enumeration gain is not a 70.6% whole-file-export improvement. It isolates
page enumeration and excludes publication/annotation work. Deep OFFSET
traversal remains a cost. Page recovery removes the redundant global count
and failed page fetch; it retains live counts and snippets.

The production SQL profiler also checks the new selected-note source header
against independently loaded reference fields. Its header takes 30 SQLite VM
steps, compared with 64 for the old preview plus 12 for the separate Feed
source query. These counts exclude the other annotation/reference reads and
measure virtual-machine work, not processor instructions or display latency.
All six query cases have five measured process means after one warmup, with
4,000 checked iterations per query/process. Full phase observations are kept.

## Validation and reproduction

- Root `make check`: 1,192 native cases passed, zero failed, 34 HTTP ignored;
  142 Python tooling cases, formatting, licenses, SQL ownership, architecture,
  strict all-target Clippy and checked benchmark workloads passed.
- The 65 affected native cases passed in default and minimal builds. New
  scenarios compare every preview field with the SQL reference, preserve
  Unicode/NUL and complete reference state, check export ordering/limits and
  deletion/undo, and distinguish clamped from exact pagination.
- The SQL profiler then gained the additional header case. Both new/updated
  focused dev benchmarks and strict default/all-target and minimal/affected
  Clippy passed after that benchmark-only addition.
- Optimized correctness mode and all production timing runs passed. No new
  HTTP, Foot or `make check-release` qualification is claimed for this change.

```sh
cargo +1.99.1 bench --locked --bench reader -- --notebook-only
cargo +1.99.1 bench --locked --bench reader -- --notebook-redundancy-only
cargo +1.99.1 bench --locked --bench reader -- --notebook-profile-only
```

Add `--test` after the workload flag for a short checked pass. For a new binary
comparison preserve the original executable, run one warmup each, then repeat
baseline/candidate/candidate/baseline three times serially. Keep mixed results.

The candidate was built from `fc4ed3c` plus the recorded source changes. Its
source hashes match accepted commit `609ef157dd678e530cbca2abed2cac85271fc046`.
That later Git revision was not embedded in the already-built benchmark. Versioned evidence:

- [Previous-runtime comparison](data/redundancy-2026-11-13-current.csv).
- [Original-baseline comparison](data/redundancy-2026-11-13-original.csv).
- [Same-process API comparison](data/redundancy-2026-11-13-local.csv).
- [SQL stage observations](data/redundancy-2026-11-13-phases.csv).
- [Provenance, medians and qualification](data/redundancy-2026-11-13-summary.json).

Temporary binaries, full logs, drivers and raw process output remain under
`/tmp/lume-redundancy-review`; they are not permanent Git artifacts. The
production profiler and repeated-work fixture are versioned in the benchmark
suite. Existing frozen baseline executables were not modified.
