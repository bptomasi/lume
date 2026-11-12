# SQL ownership

Persistence statements and SQL templates belong in `src/store/sql/`, grouped
by the capability that owns their bindings, transactions and row decoding.
`src/store` loads them with `include_str!`; this includes short SELECTs, PRAGMA
reads, undo history and predicate fragments. Existing shared queries and schema
migrations retain their paths. Migration contents and checksums are immutable.

A template may contain Rust format placeholders. Its caller supplies explicit
named arguments and retains binding order. Whitespace is part of the prepared
statement cache key, so keep template bytes stable and review composition sites
before trimming fragments or adding a final newline. Literal contents and
fragment separators retain their exact bytes. Insignificant final indentation
from a raw Rust string may be removed when it is outside SQL literals; a
preceding newline still separates any following fragment.
Ordinary string concatenation separators without SQL syntax may stay in Rust.
SQLite's typed `pragma_update` API receives pragma names and values rather than
statement text. These API arguments are not SQL templates.

Test fault injection belongs to its test owner. Store unit tests can use store
SQL fixtures. Application unit tests use `src/app/sql/<owner>/test_*.sql`;
integration tests and benchmarks use adjacent `sql/<owner>/` directories.
Production targets must never include files named `test_*.sql`. External tests
and benchmarks may reuse store schema and fault fixtures to qualify persistence
or migrations. Application unit tests own their fault SQL instead of importing
store fixtures or adding a production connection-access API.

`make check-sql`, included in `make check`, runs `tools/sql_ownership.py`:

- Every production string in `src/store/**/*.rs` is checked for SQL statements,
  composing clauses and qualified predicates. Direct string arguments to SQLite
  query/prepare/execute methods are rejected even without recognized SQL syntax.
  Short statements and lowercase SQL receive the same check as multiline SQL.
- SQL includes throughout `src/**/*.rs`, including unit-test items, must resolve
  inside the top-level source owner's `sql/` tree. For example, both
  `src/store.rs` and `src/store/query.rs` own `src/store/sql/`. Application
  modules own `src/app/sql/`. Paths are normalized and symlinks are resolved;
  traversal or a cross-owner import fails even when its spelling contains
  `/sql/`. Production includes of `test_*.sql` fail.
- Comments and literals are lexed separately. Explicit `#[cfg(test)]`/`#[test]`
  items and child modules inherit test context, including inline modules;
  literal `#[path]` attributes are supported across visibility and other
  attributes. A file reached by production and tests is
  production. An unreferenced file is conservatively production, regardless of
  its filename. Other conditional configurations are treated as production.

Literal paths and `concat!` with `env!("CARGO_MANIFEST_DIR")` are evaluated for
all Rust macro delimiters. Test filename checks apply to the included spelling
and the resolved symlink target.
Unsupported dynamic SQL include expressions fail explicitly. Other known file
extensions, such as build-generated syntax packs, are outside the SQL contract.

The inspector is intentionally lexical, not a Rust parser or SQL parser. It does
not expand arbitrary macros or infer capability ownership from table names.
It checks the repository's ordinary module declarations; nonstandard module
syntax needs an explicit extension with failure tests. It does not inspect
build-generated Rust or files outside `src`; test/benchmark SQL ownership is
checked by review.

Search compilers in `src/search/compile.rs` and `src/search/notes.rs` generate
predicates recursively from a validated AST, numbering parameters and collecting
bound values. These compiler fragments remain with that responsibility outside
the persistence-source check. They neither open connections nor execute SQL.
This is an explicit compiler boundary, not a store-file exception. Ranking
inputs and caller composition remain bound or use fixed compiler-owned
identifiers. Store snippet queries also generate numbered parameter markers;
this binding syntax contains no statement or predicate text.
