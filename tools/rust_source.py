"""Small Rust lexical inspection helpers for repository architecture checks.

This is not a Rust parser: comments, literal boundaries and balanced delimiters
suffice for our source conventions. Attribute handling recognizes explicit
`cfg(test)` and `#[test]` items and their inline/out-of-line child modules. Other
conditional configurations remain production for a conservative check.
"""

from dataclasses import dataclass, replace
import re


@dataclass(frozen=True)
class Token:
    text: str
    start: int
    end: int
    string: bool = False
    test: bool = False


def literal_end(source, start):
    """Return a Rust string's end, or None when this is not a string."""
    raw = re.match(r'(?:br|cr|r)(#+)?"', source[start:])
    if raw:
        closing = '"' + (raw[1] or '')
        end = source.find(closing, start + raw.end())
        if end < 0:
            raise ValueError("Unclosed raw string")
        return end + len(closing)
    quoted = re.match(r'(?:b|c)?"', source[start:])
    if not quoted:
        return None
    cursor = start + quoted.end()
    while cursor < len(source):
        if source[cursor] == '\\':
            cursor += 2
        elif source[cursor] == '"':
            return cursor + 1
        else:
            cursor += 1
    raise ValueError("Unclosed string")


def comment_end(source, start):
    if source.startswith('//', start):
        end = source.find('\n', start)
        return len(source) if end < 0 else end
    if not source.startswith('/*', start):
        return None
    cursor, depth = start + 2, 1
    while depth and cursor < len(source):
        if source.startswith('/*', cursor):
            depth += 1
            cursor += 2
        elif source.startswith('*/', cursor):
            depth -= 1
            cursor += 2
        else:
            cursor += 1
    if depth:
        raise ValueError("Unclosed block comment")
    return cursor


def tokens(source):
    """Tokenize code while excluding comments and preserving literal spans."""
    result, cursor = [], 0
    while cursor < len(source):
        if source[cursor].isspace():
            cursor += 1
            continue
        end = comment_end(source, cursor)
        if end is not None:
            cursor = end
            continue
        end = literal_end(source, cursor)
        if end is not None:
            result.append(Token(source[cursor:end], cursor, end, True))
            cursor = end
            continue
        char = re.match(
            r"(?:b)?'(?:\\(?:u\{[0-9a-fA-F_]+\}|.|x[0-9a-fA-F]{2})|"
            r"[^'\\\n])'", source[cursor:],
        )
        word = re.match(r'[A-Za-z_][A-Za-z_0-9]*', source[cursor:])
        match = char or word
        end = cursor + (match.end() if match else 1)
        result.append(Token(source[cursor:end], cursor, end))
        cursor = end
    return result


def closing_index(items, start):
    """Find a balanced delimiter's closing token."""
    pairs = {'(': ')', '[': ']', '{': '}'}
    stack = [pairs[items[start].text]]
    for index in range(start + 1, len(items)):
        value = items[index].text
        if items[index].string:
            continue
        if value in pairs:
            stack.append(pairs[value])
        elif value in pairs.values():
            if value != stack.pop():
                raise ValueError("Unbalanced Rust delimiters")
            if not stack:
                return index
    raise ValueError("Unclosed Rust delimiter")


def test_contexts(items, inherited=False):
    """Mark explicit test-only items without treating comments as attributes."""
    result = list(items)
    cursor = 0
    while cursor < len(items):
        item = items[cursor]
        if item.text != '#' or cursor + 1 == len(items):
            cursor += 1
            continue
        if items[cursor + 1].text != '[':
            cursor += 1
            continue
        end = closing_index(items, cursor + 1)
        attribute = ''.join(token.text for token in items[cursor + 2:end])
        if attribute not in ('cfg(test)', 'test'):
            cursor = end + 1
            continue
        # Subsequent attributes and the item's signature precede its body/';'.
        finish = end + 1
        while finish < len(items):
            value = items[finish].text
            if value in ('[', '('):
                finish = closing_index(items, finish)
            elif value == '{':
                finish = closing_index(items, finish)
                break
            elif value == ';':
                break
            finish += 1
        for index in range(cursor, min(finish + 1, len(items))):
            result[index] = replace(result[index], test=True)
        cursor = finish + 1
    if inherited:
        return [replace(item, test=True) for item in result]
    return result


def opening_index(items, end):
    """Find a closing delimiter's opening token in an already lexed item."""
    opening = {')': '(', ']': '[', '}': '{'}
    stack = [opening[items[end].text]]
    for index in range(end - 1, -1, -1):
        value = items[index].text
        if items[index].string:
            continue
        if value in opening:
            stack.append(opening[value])
        elif value in opening.values():
            if value != stack.pop():
                raise ValueError("Unbalanced Rust delimiters")
            if not stack:
                return index
    raise ValueError("Unopened Rust delimiter")


def module_path(items, index):
    """Read a module's path attribute across visibility and other attributes."""
    start = index
    if start and items[start - 1].text == ')':
        start = opening_index(items, start - 1)
    if start and items[start - 1].text == 'pub':
        start -= 1
    while start and items[start - 1].text == ']':
        end = start - 1
        start = opening_index(items, end)
        if not start or items[start - 1].text != '#':
            break
        attribute = items[start + 1:end]
        if ([item.text for item in attribute[:2]] == ['path', '=']
                and len(attribute) == 3 and attribute[2].string):
            return string_value(attribute[2])
        start -= 1
    return None


def module_children(path, items, directory=None, attribute_directory=None):
    """Resolve child modules, including visibility and inline module paths."""
    if directory is None:
        directory = (path.parent if path.stem in ('mod', 'lib', 'main')
                     else path.with_suffix(''))
    if attribute_directory is None:
        attribute_directory = path.parent
    index = 0
    while index < len(items) - 2:
        item = items[index]
        if item.text != 'mod':
            index += 1
            continue
        name, delimiter = items[index + 1].text, items[index + 2].text
        override = module_path(items, index)
        if delimiter == '{':
            end = closing_index(items, index + 2)
            child_directory = (directory / name if override is None
                               else attribute_directory / override)
            yield from module_children(path, items[index + 3:end],
                                       child_directory, child_directory)
            index = end + 1
            continue
        if delimiter == ';':
            candidates = [directory / f'{name}.rs', directory / name / 'mod.rs']
            if override is not None:
                candidates = [attribute_directory / override]
            for child in candidates:
                if child.is_file():
                    yield child.resolve(), item.test
                    break
        index += 1


def module_contexts(parsed):
    """Propagate test status; production reachability wins for shared files."""
    children = {
        path: list(module_children(path, test_contexts(items)))
        for path, items in parsed.items()
    }
    referenced = {child for edges in children.values() for child, _ in edges}
    roots = set(parsed) - referenced
    roots.update(path for path in parsed if path.name in ('lib.rs', 'main.rs'))
    pending = [(path, False) for path in roots]
    visited = set()
    while pending:
        path, test = pending.pop()
        if (path, test) in visited:
            continue
        visited.add((path, test))
        pending.extend((child, test or child_test)
                       for child, child_test in children.get(path, []))
    return {path for path, test in visited
            if test and (path, False) not in visited}


def sources(root):
    """Inspect every src file; unreferenced files stay production by default."""
    root = root.resolve()
    if not (root / 'src').is_dir():
        raise ValueError(f'No source directory: {root / "src"}')
    paths = sorted((root / 'src').rglob('*.rs'))
    parsed = {path: tokens(path.read_text()) for path in paths}
    test_files = module_contexts(parsed)
    return {
        path: test_contexts(items, path in test_files)
        for path, items in parsed.items()
    }


def string_value(token):
    """Decode Rust escapes needed for literal inspection."""
    value = token.text
    raw = re.match(r'(?:br|cr|r)(#+)?"', value)
    if raw:
        return value[raw.end():-len('"' + (raw[1] or ''))]
    value = value[value.index('"') + 1:-1]
    escapes = {
        'n': '\n', 'r': '\r', 't': '\t', '0': '\0',
        '\\': '\\', '"': '"', "'": "'",
    }

    def decode(match):
        escaped = match[1]
        if escaped.startswith('x'):
            return chr(int(escaped[1:], 16))
        if escaped.startswith('u{'):
            return chr(int(escaped[2:-1].replace('_', ''), 16))
        if escaped[0].isspace():
            return ''
        return escapes[escaped]

    pattern = r'\\(x[0-9a-fA-F]{2}|u\{[0-9a-fA-F_]+\}|\s+|.)'
    return re.sub(pattern, decode, value)
