"""Pure bounded command/request plan for Optimal match capture; no execution."""
from pathlib import Path
import hashlib
import json
import struct
from oracles.optimal_match_recipes import recipes, manifest
from oracles.optimal_matches import REVISION, require, parse_packet

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = 'tests/fixtures/reference-optimal-match-recipes/recipes.json'
HELPER = 'scripts/oracles/optimal_matches_v1.c'
INPUTS = ['scripts/optimal_match_capture_plan.py', 'scripts/capture_optimal_matches.py',
          'scripts/check_optimal_match_capture.py', 'scripts/check_optimal_match_capture_plan.py',
          'scripts/oracles/optimal_matches.py', 'scripts/oracles/optimal_match_recipes.py',
          'scripts/oracles/optimal_match_validation.py', 'scripts/oracles/tree_validation.py',
          'scripts/oracles/tree_direct.py', HELPER, MANIFEST,
          'docs/reference-optimal-match-protocol.md', 'docs/reference-optimal-match-adapter-review.md',
          'tests/upstream/manifest.json']


def sha(data):
    return hashlib.sha256(data).hexdigest()


def source_manifest():
    result = {}
    for name in INPUTS:
        p = ROOT / name
        require(p.is_file() and not p.is_symlink(), 'regular input: ' + name)
        result[name] = sha(p.read_bytes())
    require((ROOT / MANIFEST).read_bytes() == (json.dumps(manifest(), sort_keys=True, indent=2) + '\n').encode(),
            'frozen source-only manifest')
    require(result['tests/upstream/manifest.json'] == '3bb5ac1c2a0407b71c5e5dbaabac6b39074f5b7f851801a2056856a910f3ccaa', 'pinned reference inventory')
    return result


def malformed_packets():
    p = recipes()[0].request.packet()
    n = len(recipes()[0].request.source)
    start = 44 + n
    result = {'short': p[:43], 'truncated': p[:-1], 'trailing': p + b'!', 'magic': b'!' + p[1:]}
    fields = [('source-limit', 8, 16385), ('zero-count', 12, 0), ('count-limit', 12, 65),
              ('window', 16, 9), ('hash', 20, 13), ('chain', 24, 5), ('search', 28, 0),
              ('minimum', 32, 2), ('target', 36, 131073), ('short-log', 40, 0),
              ('query-end', start, n - 7), ('repeat0', start + 4, 3),
              ('repeat1', start + 8, 0xffffffff), ('repeat2', start + 12, 3),
              ('literal-zero', start + 16, 2), ('short-beat', start + 20, 2),
              ('tail-beat', start + 20, n + 1), ('duplicate-query', start + 24, 0)]
    for name, offset, value in fields:
        result[name] = p[:offset] + struct.pack('<I', value) + p[offset + 4:]
    for name, packet in result.items():
        try:
            parse_packet(packet)
        except ValueError:
            pass
        else:
            raise ValueError('negative packet unchanged/valid: ' + name)
    return result


def requests():
    positive = {r.name: r.request.packet() for r in recipes()}
    return {**positive, **{'negative-' + n: p for n, p in malformed_packets().items()}}


def reference_sources(upstream):
    catalog = json.loads((ROOT / 'tests/upstream/manifest.json').read_bytes())
    require(catalog['upstream']['revision'] == REVISION, 'reference revision')
    sources = {r['path']: r['sha256'] for r in catalog['sources'] + catalog['excluded_sources']
               if r['path'].startswith('lib/') and Path(r['path']).suffix in ('.c', '.h')}
    for name, digest in sources.items():
        p = upstream / name
        require(p.is_file() and not p.is_symlink() and sha(p.read_bytes()) == digest, 'pinned reference source: ' + name)
    units = sorted(str(p.relative_to(upstream)) for folder in ('common', 'compress', 'decompress')
                   for p in (upstream / 'lib' / folder).glob('*.c'))
    require(len(units) == 26 and all(p in sources for p in units), '26 used codec units')
    return sources, [p for p in units if p != 'lib/compress/zstd_opt.c']


def plan(upstream, output, compiler, units):
    helper = ROOT / HELPER
    common = [str(compiler), '-std=c11', '-g', '-DDEBUGLEVEL=2', '-DZSTD_DISABLE_ASM',
              '-DZSTD_LEGACY_SUPPORT=0', '-UZSTD_MULTITHREAD', '-DXXH_NAMESPACE=ZSTD_',
              '-I', str(upstream / 'lib'), '-I', str(upstream / 'lib/common'), '-I', str(upstream / 'lib/compress')]
    rows = []

    def add(label, argv, expected=0):
        rows.append(dict(label=label, argv=[str(v) for v in argv], expected_exit=expected,
                         timeout_seconds=120, stdout_limit=(64 << 20) if '-run-om_' in label else (2 << 20), stderr_limit=1 << 20))

    add('reference-revision', ['git', '-C', upstream, 'rev-parse', 'HEAD'])
    add('reference-status', ['git', '-C', upstream, 'status', '--porcelain', '--untracked-files=no'])
    add('compiler-version', [compiler, '--version'])
    add('compiler-target', [compiler, '-dumpmachine'])
    for mode in ('normal', 'sanitized'):
        flags = ['-O2'] if mode == 'normal' else ['-O1', '-fsanitize=address,undefined',
                 '-fno-sanitize-recover=all', '-fno-omit-frame-pointer']
        base = common + flags
        add(mode + '-dependencies', base + ['-MM', helper, *[upstream / p for p in units]])
        add(mode + '-predefines', base + ['-dM', '-E', helper])
        objects = []
        for index, unit in enumerate([*[upstream / p for p in units], helper]):
            obj = output / ('c-' + mode) / f'unit-{index}.o'
            args = base + (['-Wall', '-Wextra', '-Werror'] if unit == helper else [])
            add(mode + f'-compile-{index}', args + ['-c', unit, '-o', obj])
            objects.append(obj)
        executable = output / ('c-' + mode) / 'optimal-match'
        add(mode + '-link', base + [*objects, '-o', executable])
        for name in requests():
            add(mode + '-run-' + name, [executable, output / 'requests' / (name + '.bin')],
                1 if name.startswith('negative-') else 0)
    require(len({r['label'] for r in rows}) == len(rows), 'unique command labels')
    return rows


def validate_plan(rows, upstream, output, compiler, units):
    expected = plan(upstream, output, compiler, units)
    require(json.dumps(rows, sort_keys=True) == json.dumps(expected, sort_keys=True),
            'exact reviewed command inventory')
    return len(expected)
