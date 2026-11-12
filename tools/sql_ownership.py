"""Reject inline store SQL and production inclusion of test SQL.

Run from any directory; --root exists for isolated regression fixtures. See
`docs/sql-ownership.md` for source conventions and the compiler boundary.
"""

import argparse
from pathlib import Path
import re
import sys

from rust_source import closing_index, sources, string_value


# Match statements and composing clauses, including short queries and lowercase
# SQL. Standalone pragma_update values such as "DELETE" are not SQL statements.
SQL = re.compile(
    r'\b(?:SELECT\s+(?:[\s\S]+?\s+FROM\b|'
    r'(?:\d+|NULL|TRUE|FALSE)(?=\s*(?:$|[,;)]))|[\'?:]|\w+\s*\()|'
    r'INSERT\s+(?:OR\s+\w+\s+)?INTO\b|'
    r'UPDATE\s+\w+\s+SET\b|DELETE\s+FROM\b|REPLACE\s+INTO\b|'
    r'PRAGMA\s+\w|(?:CREATE|DROP|ALTER)\s+'
    r'(?:TEMP\s+|TEMPORARY\s+|UNIQUE\s+)?'
    r'(?:TABLE|VIEW|INDEX|TRIGGER|VIRTUAL\s+TABLE)\b|'
    r'BEGIN(?:\s+(?:IMMEDIATE|DEFERRED|EXCLUSIVE))?\s*$|'
    r'(?:COMMIT|ROLLBACK)\s*;?\s*$|'
    r'WITH\s+\w+\s+AS\b|WHERE\s+\S|ORDER\s+BY\b|GROUP\s+BY\b|'
    r'(?:AND|OR)\s+\(|EXISTS\s*\(|LIMIT\s+[?:\d])|'
    r'\b[a-z]\.[a-z_]+\s*(?:=|>|<|IN\b)',
    re.IGNORECASE,
)
SQL_METHODS = {
    'execute', 'execute_batch', 'prepare', 'prepare_cached', 'query_row',
    'query_row_and_then', 'query', 'query_map',
}


def sql_argument(items, index):
    """Catch direct SQL method arguments even without recognized keywords."""
    if index < 3:
        return False
    prefix = items[index - 3:index]
    return (
        prefix[0].text == '.'
        and prefix[1].text in SQL_METHODS
        and prefix[2].text == '('
    )


def includes(items):
    """Yield include macros and their unevaluated path arguments."""
    for index, item in enumerate(items[:-2]):
        if item.text not in ('include', 'include_str', 'include_bytes'):
            continue
        if (items[index + 1].text != '!'
                or items[index + 2].text not in ('(', '[', '{')):
            continue
        end = closing_index(items, index + 2)
        yield item, items[index + 3:end]


def include_path(items, root):
    """Evaluate literal paths and concat! with Cargo's manifest directory."""
    if items and items[-1].text == ',':
        items = items[:-1]
    if len(items) == 1 and items[0].string:
        return string_value(items[0])
    if len(items) < 4 or items[1].text != '!':
        raise ValueError('unsupported include path expression')
    if items[0].text == 'env':
        if (len(items) == 5 and items[3].string
                and string_value(items[3]) == 'CARGO_MANIFEST_DIR'):
            return str(root)
        raise ValueError('only CARGO_MANIFEST_DIR is supported in SQL paths')
    if (items[0].text != 'concat'
            or items[2].text not in ('(', '[', '{')):
        raise ValueError('unsupported include path macro')
    pieces, cursor = [], 3
    for index in range(3, len(items) - 1):
        if items[index].text != ',':
            continue
        # env! contains no commas; nested concat! is intentionally unsupported.
        pieces.append(include_path(items[cursor:index], root))
        cursor = index + 1
    if cursor < len(items) - 1:
        pieces.append(include_path(items[cursor:-1], root))
    return ''.join(pieces)


def sql_directory(root, path):
    """The top-level source module owns its sql/ tree, including unit tests."""
    relative = path.relative_to(root / 'src')
    first = relative.parts[0]
    if len(relative.parts) > 1:
        return root / 'src' / first / 'sql'
    if path.stem in ('lib', 'main'):
        return root / 'src/sql'
    return root / 'src' / path.stem / 'sql'


def include_errors(root, path, source, items):
    errors = []
    for item, arguments in includes(items):
        line = source.count('\n', 0, item.start) + 1
        location = f'{path.relative_to(root)}:{line}'
        try:
            name = include_path(arguments, root)
        except ValueError as error:
            tail = ''.join(string_value(arg) for arg in arguments if arg.string)
            if Path(tail).suffix.lower() in ('', '.sql'):
                errors.append(f'{location}: SQL include path: {error}')
            continue
        if Path(name).suffix.lower() != '.sql':
            continue
        resolved = (path.parent / name).resolve()
        if not resolved.is_relative_to(sql_directory(root, path).resolve()):
            errors.append(f'{location}: SQL include leaves owner sql/: {name}')
        test_names = (Path(name).name, resolved.name)
        if (not item.test
                and any(name.startswith('test_') for name in test_names)):
            errors.append(
                f'{location}: production includes test SQL: {name}'
            )
    return errors


def violations(root):
    root = root.resolve()
    errors = []
    for path, items in sources(root).items():
        relative = path.relative_to(root)
        source = path.read_text()
        owned = (relative == Path('src/store.rs')
                 or relative.parts[:2] == ('src', 'store'))
        for index, item in enumerate(items):
            if item.test or not item.string or not owned:
                continue
            value = string_value(item)
            if SQL.search(value) or sql_argument(items, index):
                line = source.count('\n', 0, item.start) + 1
                errors.append(f'{relative}:{line}: inline store SQL; use sql/')
        errors.extend(include_errors(root, path, source, items))
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path,
                        default=Path(__file__).resolve().parents[1])
    arguments = parser.parse_args()
    try:
        errors = violations(arguments.root.resolve())
    except (OSError, ValueError, KeyError, IndexError) as error:
        print(f'SQL ownership inspection failed: {error}', file=sys.stderr)
        return 1
    for error in errors:
        print(error, file=sys.stderr)
    if errors:
        return 1
    print('SQL ownership: store queries and production includes passed')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
