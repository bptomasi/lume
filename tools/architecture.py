"""Conservative checks of declared Rust ownership, not a Rust type checker.

Nodes are the root modules of each real crate, plus its composition root.
Other cfg predicates are treated as production. Imports, reexports and type
aliases are resolved lexically; macro expansion and inferred receiver types
remain the compiler's responsibility. Unreachable source is rejected rather
than silently omitted from the production graph.
"""

import argparse
from dataclasses import dataclass
from pathlib import Path
import re

from rust_source import (
    closing_index, module_path, opening_index, sources, test_contexts, tokens,
)


IDENTIFIER = re.compile(r'[A-Za-z_][A-Za-z_0-9]*')
MEM_FUNCTIONS = {('<external>', root, 'mem', operation)
                 for root in ('std', 'core')
                 for operation in ('take', 'replace')}


@dataclass
class Module:
    name: tuple
    path: Path
    items: list

    @property
    def owner(self):
        return self.name[0] if self.name else '<composition>'


def load_modules(root):
    """Retain logical module paths, including inline and #[path] modules."""
    modules, visited = {}, set()

    def visit(path, name, items, directory=None, attribute_directory=None):
        visited.add(path)
        own, index = [], 0
        while index < len(items):
            if (items[index].text != 'mod' or index + 2 >= len(items)
                    or items[index + 2].text not in (';', '{')):
                own.append(items[index])
                index += 1
                continue
            end = module_end(items, index)
            if not items[index].test:
                visit_child(path, name, items, index, end, directory,
                            attribute_directory, visit)
            index = end + 1
        modules[name] = Module(name, path, own)

    path = root.resolve()
    visit(path, (), test_contexts(tokens(path.read_text())), path.parent)
    return modules, visited


def module_end(items, index):
    if items[index + 2].text == '{':
        return closing_index(items, index + 2)
    return index + 2


def visit_child(path, name, items, index, end, directory,
                attribute_directory, visit):
    child_name = items[index + 1].text
    directory = directory or (path.parent if path.stem in ('lib', 'main', 'mod')
                              else path.with_suffix(''))
    override = module_path(items, index)
    if items[index + 2].text == ';':
        candidates = [directory / f'{child_name}.rs',
                      directory / child_name / 'mod.rs']
        if override is not None:
            candidates = [(attribute_directory or path.parent) / override]
        child = next((p.resolve() for p in candidates if p.is_file()), None)
        if child is None:
            raise ValueError(f'{path}: unresolved module {child_name}')
        visit(child, (*name, child_name),
              test_contexts(tokens(child.read_text())))
        return
    child_directory = directory / child_name
    if override is not None:
        child_directory = (attribute_directory or path.parent) / override
    visit(path, (*name, child_name), items[index + 3:end],
          child_directory, child_directory)


def use_paths(items, prefix=()):
    """Flatten grouped import trees without discarding aliases or globs."""
    index, path = 0, list(prefix)
    while index < len(items):
        value = items[index].text
        if value == '{':
            end = closing_index(items, index)
            yield from use_paths(items[index + 1:end], tuple(path))
            path = list(prefix)
            index = end + 1
        elif value == ',':
            if path != list(prefix):
                yield import_binding(path)
            path = list(prefix)
            index += 1
        elif value == ':':
            index += 1
        else:
            path.append(value)
            index += 1
    if path != list(prefix):
        yield import_binding(path)


def import_binding(path):
    if 'as' in path:
        split = path.index('as')
        return path[split + 1], tuple(path[:split])
    if path[-1] == 'self':
        path = path[:-1]
    return path[-1], tuple(path)


def public_item(items, index):
    if index and items[index - 1].text == ')':
        index = opening_index(items, index - 1)
    return index > 0 and items[index - 1].text == 'pub'


def imports(items, exported=False):
    for index, item in enumerate(items):
        if item.text != 'use' or item.test:
            continue
        if exported and not public_item(items, index):
            continue
        end = index + 1
        while end < len(items) and items[end].text != ';':
            end += 1
        yield from use_paths(items[index + 1:end])


def definitions(module, exported=False):
    """Yield named definitions and type bodies, excluding impl blocks."""
    items = module.items
    kinds = {'struct', 'enum', 'trait', 'type', 'fn', 'const', 'static'}
    for index, item in enumerate(items[:-1]):
        if item.test or item.text not in kinds:
            continue
        if exported and not public_item(items, index):
            continue
        name = items[index + 1].text
        if not IDENTIFIER.fullmatch(name):
            continue
        body, end = [], index + 2
        while end < len(items):
            value = items[end].text
            if value in ('{', '('):
                finish = closing_index(items, end)
                if item.text in ('struct', 'enum', 'type'):
                    body.extend(items[end + 1:finish])
                end = finish
                if value == '{':
                    break
            elif value == ';':
                break
            elif item.text in ('struct', 'enum', 'type'):
                body.append(items[end])
            end += 1
        yield (*module.name, name), body


class Resolver:
    """Resolve paths while recording intermediate reexport dependencies.

    Canonicalizing to a definition must not erase the module that exports it:
    app -> service::Backref -> app::App is still a cycle.
    """

    def __init__(self, modules, library):
        self.modules = modules
        self.library = library
        self.bindings = {
            name: {alias: path for alias, path in imports(module.items)
                   if alias != '*'} for name, module in modules.items()
        }
        self.globs = {name: [path[:-1] for alias, path in imports(module.items)
                            if alias == '*']
                      for name, module in modules.items()}
        self.symbols = {name for module in modules.values()
                        for name, _ in definitions(module)} | set(modules)
        self.reached = set()

    def resolve(self, module, path, seen=frozenset()):
        if not path:
            return module
        key = (module, path)
        if key in seen:
            return path
        seen = seen | {key}
        head, *tail = path
        if head == 'crate' or (head == 'lume' and self.library):
            return self.expand(tuple(tail), seen)
        if head == 'self':
            return self.expand((*module, *tail), seen)
        if head == 'super':
            return self.resolve(module[:-1], tuple(tail), seen)
        binding = self.bindings.get(module, {}).get(head)
        if binding is not None:
            target = self.resolve(module, binding, seen)
            return self.expand((*target, *tail), seen)
        local = (*module, head)
        if local in self.symbols:
            return self.expand((*local, *tail), seen)
        candidate = self.resolve_globs(module, head, seen)
        if candidate is not None:
            return self.expand((*candidate, *tail), seen)
        return ('<external>', *path)

    def resolve_globs(self, module, head, seen):
        for glob in self.globs.get(module, []):
            target = self.resolve(module, glob, seen)
            if target in self.modules:
                candidate = self.resolve(target, (head,), seen)
                if candidate and (candidate[0] != '<external>'
                                  or candidate in MEM_FUNCTIONS):
                    return candidate
            # These external names have known semantics for the Copy guard.
            if (target in (('<external>', 'std', 'mem'),
                           ('<external>', 'core', 'mem'))
                    and head in ('take', 'replace')):
                return (*target, head)
        return None

    def glob_members(self, module, seen=frozenset()):
        """Expose declared local names, including transitive glob reexports."""
        if module in seen or module not in self.modules:
            return
        seen = seen | {module}
        source = self.modules[module]
        names = {path[-1] for path, _ in definitions(source, exported=True)}
        exports = list(imports(source.items, exported=True))
        names.update(alias for alias, _ in exports if alias != '*')
        for name in sorted(names):
            yield self.resolve(module, (name,))
        for alias, path in exports:
            if alias == '*':
                target = self.resolve(module, path[:-1])
                yield from self.glob_members(target, seen)

    def expand(self, path, seen):
        if path and (path[0],) in self.modules:
            self.reached.add(path[0])
        elif path and path[:1] in self.symbols:
            self.reached.add('<composition>')
        for split in range(len(path) - 1, -1, -1):
            prefix, suffix = path[:split], path[split:]
            if prefix in self.modules:
                resolved = self.resolve(prefix, suffix, seen)
                if (resolved and resolved[0] == '<external>'
                        and suffix[0] not in self.bindings[prefix]
                        and resolved not in MEM_FUNCTIONS):
                    return path
                if resolved != suffix:
                    return resolved
        return path


def references(module, resolver, items=None):
    items = module.items if items is None else items
    index = 0
    while index < len(items):
        item = items[index]
        if (item.test or item.string
                or not IDENTIFIER.fullmatch(item.text)):
            index += 1
            continue
        path, end = [item.text], index + 1
        while (end + 2 < len(items)
               and [token.text for token in items[end:end + 2]] == [':', ':']
               and IDENTIFIER.fullmatch(items[end + 2].text)):
            path.append(items[end + 2].text)
            end += 3
        yield resolver.resolve(module.name, tuple(path))
        index = end


def dependency_graph(modules, resolver):
    owners = {module.owner for module in modules.values()}
    graph = {owner: set() for owner in owners}
    for module in modules.values():
        resolver.reached = set()
        for path in declared_references(module, resolver):
            if path and path[0] in owners and path[0] != module.owner:
                graph[module.owner].add(path[0])
        graph[module.owner].update(resolver.reached - {module.owner})
    # Root module declarations are composition dependencies, even unused ones.
    graph['<composition>'].update(owners - {'<composition>'})
    return graph


def declared_references(module, resolver):
    yield from references(module, resolver)
    for _, path in imports(module.items):
        yield resolver.resolve(module.name, path)


def authority_references(module, resolver):
    yield from declared_references(module, resolver)
    for alias, path in imports(module.items):
        if alias == '*':
            target = resolver.resolve(module.name, path[:-1])
            yield from resolver.glob_members(target)


def cycles(graph):
    """Return back-edge paths without enumerating every dependency walk."""
    active, complete, result = [], set(), []

    def visit(owner):
        if owner in active:
            result.append(active[active.index(owner):] + [owner])
            return
        if owner in complete:
            return
        active.append(owner)
        for child in sorted(graph[owner]):
            visit(child)
        active.pop()
        complete.add(owner)

    for owner in sorted(graph):
        visit(owner)
    return result


def app_authority(modules, resolver):
    """A view/alias containing App also carries App authority transitively."""
    authority = {('app', 'App')}
    bodies = {name: (module, body) for module in modules.values()
              for name, body in definitions(module) if body}
    changed = True
    while changed:
        changed = False
        for name, (module, body) in bodies.items():
            if name not in authority and any(
                    path in authority for path in references(
                        module, resolver, body)):
                authority.add(name)
                changed = True
    return authority


def private_app_errors(modules):
    module = modules.get(('app',))
    if module is None:
        return []
    for name, body in definitions(module):
        if name == ('app', 'App') and any(t.text == 'pub' for t in body):
            return ['App root fields must remain private']
    return []


COPY_SCALARS = {
    'bool', 'char', 'u8', 'u16', 'u32', 'u64', 'u128', 'usize',
    'i8', 'i16', 'i32', 'i64', 'i128', 'isize', 'f32', 'f64',
}


def copy_getters(modules):
    """Index scalar-return signatures, preserving borrowed/unknown results."""
    signatures = {}
    for module in modules.values():
        items = module.items
        for index, item in enumerate(items[:-3]):
            if item.test or item.text != 'fn':
                continue
            name = items[index + 1].text
            end = index + 2
            while end < len(items) and items[end].text not in ('{', ';'):
                end += 1
            signature = [t.text for t in items[index + 2:end]]
            if 'where' in signature:
                signature = signature[:signature.index('where')]
            returns_copy = (len(signature) >= 3
                            and signature[-3:-1] == ['-', '>']
                            and signature[-1] in COPY_SCALARS)
            signatures.setdefault(name, set()).add(returns_copy)
    return {name for name, results in signatures.items() if results == {True}}


def temporary_errors(module, resolver, getters):
    items, errors = module.items, []
    for index, item in enumerate(items):
        if item.test or item.text not in ('take', 'replace') and not (
                item.text in resolver.bindings.get(module.name, {})):
            continue
        operation = mem_operation(module, resolver, index)
        if operation is None:
            continue
        opening = call_opening(items, index + 1)
        if opening is None:
            continue
        borrow = [t.text for t in items[opening + 1:opening + 3]]
        if borrow != ['&', 'mut']:
            continue
        end = closing_index(items, opening)
        getter = temporary_getter(items[opening + 3:end], getters)
        if getter is not None:
            errors.append(f'{module.path}: mem::{operation} mutates '
                          f'a Copy getter temporary ({getter})')
    return errors


def mem_operation(module, resolver, index):
    items, start = module.items, index
    while start >= 3 and items[start - 1].text == ':':
        start -= 3
    path = tuple(t.text for t in items[start:index + 1] if t.text != ':')
    target = resolver.resolve(module.name, path)
    if target and target[0] == '<external>':
        target = target[1:]
    if target in (('std', 'mem', 'take'), ('std', 'mem', 'replace'),
                  ('core', 'mem', 'take'), ('core', 'mem', 'replace')):
        return target[-1]
    return None


def call_opening(items, index):
    """Locate arguments after an optional explicit generic argument list."""
    if [t.text for t in items[index:index + 3]] == [':', ':', '<']:
        index += 3
        depth = 1
        while index < len(items) and depth:
            if items[index].text == '<':
                depth += 1
            elif items[index].text == '>':
                depth -= 1
            index += 1
    return index if index < len(items) and items[index].text == '(' else None


def temporary_getter(items, getters):
    """Check the actual borrowed result, not a getter inside another call."""
    index = 0
    while index < len(items):
        if items[index].text == ',':
            items = items[:index]
            break
        if items[index].text in ('(', '[', '{'):
            index = closing_index(items, index)
        index += 1
    while (items and items[0].text == '('
           and closing_index(items, 0) == len(items) - 1):
        items = items[1:-1]
    if not items or items[-1].text != ')':
        return None
    opening = opening_index(items, len(items) - 1)
    name = called_name(items, opening)
    return name if name in getters else None


def called_name(items, opening):
    """Read a called name before an optional explicit generic argument list."""
    index = opening - 1
    if index >= 0 and items[index].text == '>':
        depth = 1
        index -= 1
        while index >= 0 and depth:
            if items[index].text == '>':
                depth += 1
            elif items[index].text == '<':
                depth -= 1
            index -= 1
        if [t.text for t in items[index - 1:index + 1]] != [':', ':']:
            return None
        index -= 2
    if index >= 0 and IDENTIFIER.fullmatch(items[index].text):
        return items[index].text
    return None


def crate_errors(root, library):
    modules, visited = load_modules(root)
    resolver = Resolver(modules, library)
    graph = dependency_graph(modules, resolver)
    errors = [f'{root.name}: production cycle: {" -> ".join(cycle)}'
              for cycle in cycles(graph)]
    if library:
        errors.extend(private_app_errors(modules))
        authority = app_authority(modules, resolver)
        for module in modules.values():
            if module.owner == 'ui' and any(
                    path in authority for path in authority_references(
                        module, resolver)):
                errors.append(f'{module.path}: UI receives App authority')
        if 'ui' in graph.get('app', set()):
            errors.append('App must not depend on UI')
    getters = copy_getters(modules)
    for module in modules.values():
        errors.extend(temporary_errors(module, resolver, getters))
    return errors, visited


def violations(root):
    errors, visited = [], set()
    for name in ('src/lib.rs', 'src/main.rs', 'build.rs'):
        entry = root / name
        if entry.is_file():
            found, reached = crate_errors(entry, name == 'src/lib.rs')
            errors.extend(found)
            visited.update(reached)
    for path, items in sources(root).items():
        if path not in visited and any(not item.test for item in items):
            errors.append(f'{path}: unreachable production source')
    return sorted(set(errors))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path,
                        default=Path(__file__).resolve().parents[1])
    arguments = parser.parse_args()
    try:
        errors = violations(arguments.root)
    except (OSError, ValueError, RecursionError) as error:
        parser.exit(1, f'Architecture inspection failed: {error}\n')
    if errors:
        parser.exit(1, '\n'.join(errors) + '\n')
    print('Production architecture, App authority and Copy getters verified')


if __name__ == '__main__':
    main()
