"""Pure bounded command/request plan for Optimal price capture; no execution."""
from pathlib import Path
import hashlib
import json
import struct
from oracles.optimal_price_recipes import recipes, manifest
from oracles.optimal_prices_v2 import REVISION, require, parse_packet

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = 'tests/fixtures/reference-optimal-price-recipes/recipes.json'
HELPER = 'scripts/oracles/optimal_price_v2.c'
INPUTS = ['scripts/optimal_price_capture_plan.py', 'scripts/capture_optimal_prices.py',
          'scripts/check_optimal_price_capture.py', 'scripts/check_optimal_price_capture_plan.py',
          'scripts/oracles/optimal_prices_v2.py', 'scripts/oracles/optimal_price_recipes.py',
          'scripts/oracles/optimal_price_validation.py', HELPER, MANIFEST,
          'docs/reference-optimal-price-protocol.md', 'docs/reference-optimal-price-adapter-review.md',
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
    r = next(r for r in recipes() if r.name == 'op_init_single_9_m0').request
    p = r.packet()
    result = {'short': p[:19], 'truncated': p[:-1], 'trailing': p + b'!', 'magic': b'!' + p[1:]}
    for name, offset, value in [('source-limit', 8, 131073), ('zero-count', 12, 0),
                                ('count-limit', 12, 129), ('mode', 16, 2),
                                ('init-tag', 29, 2), ('init-range', 33, 10),
                                ('init-reserved', 41, 1), ('query-zero-offset', 69, 0),
                                ('query-short-match', 73, 2), ('query-LL-limit', 65, 131073),
                                ('query-range', 57, 1), ('update-reserved', 97, 1)]:
        result[name] = p[:offset] + struct.pack('<I', value) + p[offset + 4:]
    result['dirty-query'] = p[:101] + p[53:77] + p[125:]
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
                         timeout_seconds=120, stdout_limit=2 << 20, stderr_limit=1 << 20))

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
        executable = output / ('c-' + mode) / 'optimal-price'
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
