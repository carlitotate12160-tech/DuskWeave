#!/usr/bin/env python3
"""Check the current protected domain/contract modules' inward dependency boundary.

This is an import/path check, not a claim to prove semantic independence or macro
expansion. Origins of aliases and re-exports are checked before names are hidden.
"""
from pathlib import Path
import re
import sys

PROTECTED = (
    'mission.rs', 'm1_permission.rs', 'm1_policy.rs', 'm1_session.rs',
    'm1_inference_permission.rs', 'm1_inference_budget.rs', 'm1_context.rs',
    'm1_context_input.rs', 'm1_context_capacity.rs', 'm1_autonomous_context.rs',
    'm1_terrain.rs', 'm1_terrain_input.rs', 'm1_terrain_view.rs', 'm1_terrain_issued.rs',
    'm1_run.rs', 'm1_reasoning/inference.rs', 'm1_reasoning/decision_attempt.rs',
    'm1_reasoning/proposal.rs', 'm1_reasoning/request.rs', 'm1_reasoning/request_autonomous.rs',
    'm1_reasoning/bounded_json.rs', 'm1_reasoning/response.rs',
)
TOKEN = re.compile(r'//[^\n]*|/\*|r(\#*)"|"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])\'|[a-zA-Z_][a-zA-Z_0-9]*|::|[{};,*]', re.S)
IDENT = re.compile(r'^[a-zA-Z_][a-zA-Z_0-9]*$')


def tokens(source):
    result, pos = [], 0
    while match := TOKEN.search(source, pos):
        token, pos = match[0], match.end()
        if token == '/*':
            depth = 1
            while depth:
                nested = re.search(r'/\*|\*/', source[pos:])
                if not nested:
                    raise ValueError('unclosed Rust comment')
                depth += 1 if nested[0] == '/*' else -1
                pos += nested.end()
        elif token.startswith('r') and token.endswith('"'):
            end = source.find('"' + match[1], pos)
            if end < 0:
                raise ValueError('unclosed Rust raw string')
            pos = end + 1 + len(match[1])
        elif not token.startswith(('//', '"', "'")):
            result.append(token)
    return result


def import_paths(items, index, prefix=()):
    path = list(prefix)
    while index < len(items):
        token = items[index]
        if IDENT.fullmatch(token) and token != 'as':
            path.append(token)
            index += 1
        elif token == '::':
            index += 1
        elif token == '{':
            paths, index = import_group(items, index + 1, tuple(path))
            return paths, index
        else:
            if token == 'as':
                index += 2  # The origin was already collected; an alias conveys no exception.
            return [tuple(path)], index
    return [tuple(path)], index


def import_group(items, index, prefix):
    paths = []
    while index < len(items) and items[index] != '}':
        found, index = import_paths(items, index, prefix)
        paths.extend(found)
        if index < len(items) and items[index] == ',':
            index += 1
        elif index < len(items) and items[index] != '}':
            raise ValueError('unsupported Rust use tree')
    return paths, index + 1


def paths(source):
    items, result = tokens(source), []
    for i, token in enumerate(items):
        if token == 'use':
            found, _ = import_paths(items, i + 1)
            result.extend(found)
        if IDENT.fullmatch(token) and i + 1 < len(items) and items[i + 1] == '::':
            path, j = [token], i + 1
            while j + 1 < len(items) and items[j] == '::' and IDENT.fullmatch(items[j + 1]):
                path.append(items[j + 1])
                j += 2
            result.append(tuple(path))
        if token == 'extern' and items[i + 1:i + 2] == ['crate']:
            result.append(tuple(items[i + 2:i + 3]))
    return result


def violations(name, source):
    errors = set()
    for path in paths(source):
        if any(p == 'postgres' or p.startswith('postgres_') or p == 'curl' or p.endswith('_cli') for p in path):
            errors.add('::'.join(path))
        if path[:2] == ('std', 'process'):
            errors.add('::'.join(path))
        if 'm1_reasoning' in path and any(p in ('transport', 'postgres_terrain') for p in path):
            errors.add('::'.join(path))
        if 'm1_run' in path and name != 'm1_run.rs':
            errors.add('::'.join(path))
        if name.startswith('m1_reasoning/') and 'transport' in path:
            errors.add('::'.join(path))
    return sorted(errors)


def main():
    root = Path(__file__).resolve().parents[1] / 'src'
    failures = []
    for name in PROTECTED:
        try:
            source = (root / name).read_text(encoding='utf-8')
            failures.extend(f'{name}: {p}' for p in violations(name, source))
        except (OSError, ValueError) as error:
            failures.append(f'{name}: {error}')
    for failure in failures:
        print(f'FAIL dependency direction {failure}')
    print(f'dependency direction: {len(PROTECTED)} protected modules, {len(failures)} violations')
    return int(bool(failures))


if __name__ == '__main__':
    sys.exit(main())
