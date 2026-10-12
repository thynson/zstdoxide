#!/usr/bin/env python3
"""Replay a qualified Greedy capture offline, including every compile/link command."""
from pathlib import Path, PurePosixPath
import argparse
import hashlib
import json
import os
import re

ROOT = Path(__file__).resolve().parents[1]
CAPTURE_SHA = '97f1a9d6fe90f16da7e2d4531eb19b389d45a391e6667f5c34d464fbaaac2255'
FIXTURE_SHA = 'f61e92df7e1b58d548b7058d30cbfb7228f3cc1e4cfd3fbad9b3050e0cc20e41'
COLLECTION = 'tests/fixtures/reference-greedy-families'
CHECKER_SHA = 'b1edd15038de0fa3b4af0d1d03d7fb15ec636f83ead060cc7578c82449c1a207'


def require(value, reason):
    if not value:
        raise ValueError(reason)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def read(root, name, cap):
    path = PurePosixPath(name)
    require(not path.is_absolute() and '..' not in path.parts and str(path) == name
            and '\\' not in name, 'canonical artifact path')
    p = root / name
    require(p.resolve().is_relative_to(root) and p.is_file() and not p.is_symlink()
            and p.stat().st_size <= cap, 'bounded regular artifact: ' + name)
    data = p.read_bytes()
    require(len(data) <= cap, 'artifact grew beyond cap')
    return data


def fixture():
    require(sha(read(ROOT, 'scripts/check_greedy_family_recipes.py', 1 << 20)) == CHECKER_SHA,
            'reviewed recipe checker identity')
    import check_greedy_family_recipes as checker
    g, cases, _ = checker.inputs()
    raw = read(ROOT, COLLECTION + '/capture.json', 1 << 20)
    require(sha(raw) == FIXTURE_SHA, 'immutable accepted capture identity')
    accepted = g.decode_json(raw)
    pack = read(ROOT, COLLECTION + '/observations.ndjson', 8 << 20)
    require(sha(pack) == accepted['pack_sha256'] and g.exact(len(pack), accepted['pack_bytes']), 'complete capture pack')
    names = [c.name for c in cases] + ['salt-normal', 'salt-sanitized']
    require(set(accepted['records']) == set(names), 'frozen observation inventory')
    cursor, observed = 0, {}
    for name in names:
        record = accepted['records'][name]
        require(g.exact(record['offset'], cursor), 'consecutive observation extent')
        size = g.integer(record['bytes'], 1, len(pack) - cursor, 'observation extent size')
        raw = pack[cursor:cursor + size]
        require(sha(raw) == record['sha256'], 'observation extent hash')
        observed[name] = raw
        cursor += size
    require(cursor == len(pack), 'no unused observation bytes')
    for case in cases:
        g.validate(case, observed[case.name])
    for mode in ('normal', 'sanitized'):
        g.validate_salt(observed['salt-' + mode])
    return accepted, observed


def audit(directory):
    for name, digest in {'scripts/capture_greedy_families.py': CAPTURE_SHA,
                         'scripts/check_greedy_family_recipes.py': CHECKER_SHA}.items():
        require(sha(read(ROOT, name, 1 << 20)) == digest, 'reviewed infrastructure identity')
    import check_greedy_family_recipes as checker
    g, cases, manifest = checker.inputs()
    accepted, frozen = fixture()
    root = directory.resolve()
    e = g.decode_json(read(root, 'evidence.json', 4 << 20))
    require(e['status'] == 'passed' and g.exact(e['schema_version'], 1) and e['upstream_revision'] == g.REVISION,
            'completed qualified capture')
    require(e['support_archive_used'] is False and e['upstream_sanitizer_annotations_retained'] is True,
            'instrumentation scope')
    local = set(checker.PINS) | {checker.MANIFEST, 'scripts/check_greedy_family_recipes.py',
                               'scripts/capture_greedy_families.py', 'tests/upstream/manifest.json'}
    source = {name: sha(read(ROOT, name, 2 << 20)) for name in sorted(local)}
    require(g.exact(e['source_before'], source) and g.exact(e['source_after'], source)
            and g.exact(accepted['source'], source), 'source closure')
    require(read(root, 'recipes.json', 1 << 20) == read(ROOT, checker.MANIFEST, 1 << 20), 'input manifest bytes')
    inventory = g.decode_json(read(ROOT, 'tests/upstream/manifest.json', 2 << 20))
    c_source = {r['path']: r['sha256'] for r in inventory['sources'] + inventory['excluded_sources']
                if r['path'].startswith('lib/') and Path(r['path']).suffix in ('.c', '.h')}
    units = sorted(n for n in c_source if n.endswith('.c') and PurePosixPath(n).parent.as_posix()
                   in ('lib/common', 'lib/compress', 'lib/decompress'))
    require(len(units) == 26 and g.exact(e['upstream_source_sha256'], c_source), 'complete pinned C inventory')
    require(set(e['builds']) == {'normal', 'sanitized'}, 'build inventory')
    require(e['environment']['ASAN_OPTIONS'] == 'detect_leaks=0:halt_on_error=1' and
            e['environment']['UBSAN_OPTIONS'] == 'halt_on_error=1:print_stacktrace=1', 'sanitizer environment')
    g.integer(len(e['commands']), 444, 444, 'command count')
    commands = {r['label']: r for r in e['commands']}
    require(len(commands) == len(e['commands']), 'unique command labels')
    expected_labels = {'reference-revision', 'reference-status', 'compiler-version', 'compiler-target'}
    artifacts, raw = set(), {}
    for label, command in commands.items():
        require('error' not in command and command['stdin'] is None, 'successful bounded file transport')
        negative = '-negative-' in label
        require(g.exact(command['expected_exit'], 1 if negative else 0) and
                g.exact(command['exit_code'], 1 if negative else 0), 'command exit')
        require(g.exact(command['timeout_seconds'], 30 if negative else 120), 'command timeout policy')
        for stream in ('stdout', 'stderr'):
            record = command[stream]
            name = f'logs/{label}.{stream}'
            require(record['path'] == name and name not in artifacts, 'log identity')
            data = read(root, name, g.MAX_RECORDS if stream == 'stdout' else 1 << 20)
            require(g.exact(record['bytes'], len(data)) and record['sha256'] == sha(data), 'log bytes/hash')
            artifacts.add(name)
            raw[label, stream] = data
        if negative:
            require(not raw[label, 'stdout'] and raw[label, 'stderr'].startswith(b'greedy oracle [setup]:'),
                    'malformed packet diagnostic')
        else:
            require(not raw[label, 'stderr'], 'unexpected diagnostics')
    require(raw['reference-revision', 'stdout'].decode().strip() == g.REVISION and
            not raw['reference-status', 'stdout'], 'unchanged reference revision')
    original_root = Path(commands['reference-revision']['cwd'])
    upstream = Path(commands['reference-revision']['argv'][2])
    output = Path(commands['normal-compile-0']['argv'][-1]).parent.parent
    cc = e['compiler']
    require(original_root.is_absolute() and upstream.is_absolute() and output.is_absolute(), 'capture locations')
    for row in commands.values():
        require(row['cwd'] == str(original_root), 'command working directory')
    require(commands['reference-revision']['argv'] == ['git', '-C', str(upstream), 'rev-parse', 'HEAD'] and
            commands['reference-status']['argv'] == ['git', '-C', str(upstream), 'status', '--porcelain', '--untracked-files=no'],
            'reference command arguments')
    require(commands['compiler-version']['argv'] == [cc, '--version'] and
            commands['compiler-target']['argv'] == [cc, '-dumpmachine'] and
            re.fullmatch('[0-9a-f]{64}', e['compiler_sha256']), 'compiler identity')
    helper = original_root / 'scripts/oracles/greedy_direct.c'
    common = [cc, '-std=c11', '-g', '-I', str(upstream / 'lib'), '-I', str(upstream / 'lib/common'),
              '-I', str(upstream / 'lib/compress'), '-DZSTD_DISABLE_ASM', '-DZSTD_LEGACY_SUPPORT=0',
              '-UZSTD_MULTITHREAD', '-DXXH_NAMESPACE=ZSTD_']
    for mode in ('normal', 'sanitized'):
        flags = ['-O2'] if mode == 'normal' else ['-O1', '-fsanitize=address,undefined',
                '-fno-sanitize-recover=all', '-fno-omit-frame-pointer']
        build, objects = e['builds'][mode], []
        require(g.exact(build['compiled_codec_sources'], units) and
                g.exact(build['instrumented_codec_sources'], units if mode == 'sanitized' else []) and
                build['helper_instrumented'] is (mode == 'sanitized') and
                build['assembly'] is False and build['workers'] is False and build['leak_sanitizer'] is False,
                'per-unit instrumentation scope')
        for i, unit in enumerate(units):
            label = f'{mode}-compile-{i}'
            obj = output / ('c-' + mode) / f'unit-{i}.o'
            expected_labels.add(label)
            require(commands[label]['argv'] == common + flags + ['-c', str(upstream / unit), '-o', str(obj)],
                    'exact compile command')
            objects.append(str(obj))
        for label in (mode + '-dependencies', mode + '-predefines', mode + '-link'):
            expected_labels.add(label)
        require(commands[mode + '-dependencies']['argv'] == common + flags + ['-MM', str(helper),
                *[str(upstream / u) for u in units]], 'dependency command')
        require(commands[mode + '-predefines']['argv'] == common + flags + ['-dM', '-E', str(helper)], 'predefines command')
        exe = output / ('c-' + mode) / 'greedy-direct'
        require(commands[mode + '-link']['argv'] == common + flags +
                ['-Wall', '-Wextra', '-Werror', str(helper), *objects, '-o', str(exe)], 'complete link inputs')
        observed_deps = {}
        for line in raw[mode + '-dependencies', 'stdout'].decode().replace('\\\n', ' ').splitlines():
            for name in line.split(':', 1)[1].split():
                path = Path(os.path.normpath(name))
                require(path == helper or path.is_relative_to(upstream), 'dependency path')
                digest = source['scripts/oracles/greedy_direct.c'] if path == helper else c_source[str(path.relative_to(upstream))]
                observed_deps[str(path)] = digest
        require(g.exact(build['dependencies'], observed_deps), 'exact dependency closure')
        expected_objects = {f'c-{mode}/unit-{i}.o' for i in range(26)}
        require(set(build['objects']) == expected_objects and all(re.fullmatch('[0-9a-f]{64}', h)
                for h in build['objects'].values()), 'object hash inventory')
        for name, digest in {**build['objects'], f'c-{mode}/greedy-direct': build['executable_sha256']}.items():
            require(re.fullmatch('[0-9a-f]{64}', digest), 'binary identity')
            if (root / name).exists():
                require(sha(read(root, name, 64 << 20)) == digest, 'retained binary hash')
        defines = raw[mode + '-predefines', 'stdout'].decode()
        require('#define ZSTD_MULTITHREAD ' not in defines and '#define ZSTD_DISABLE_ASM ' in defines,
                'disabled worker/assembly predefines')
    negatives = checker.malformed_packets(g, cases)
    packets = {c.name: c.packet() for c in cases}
    packets.update({'negative-' + n: b for n, b in negatives.items()})
    require(set(e['requests']) == set(packets), 'packet inventory')
    for name, data in packets.items():
        filename = 'requests/' + name + '.bin'
        require(g.exact(e['requests'][name], {'path': filename, 'bytes': len(data), 'sha256': sha(data)}) and
                read(root, filename, g.MAX_PACKET + 1) == data, 'exact independently rebuilt packet')
    salt_rows = {}
    for mode in ('normal', 'sanitized'):
        exe = str(output / ('c-' + mode) / 'greedy-direct')
        label = mode + '-fresh-salt'
        expected_labels.add(label)
        require(commands[label]['argv'] == [exe, '--fresh-salt'], 'fresh salt invocation')
        salt_rows[mode] = g.validate_salt(raw[label, 'stdout'])
        require(raw[label, 'stdout'] == frozen['salt-' + mode], 'frozen per-mode salt record')
        for case in cases:
            label = mode + '-case-' + case.name
            expected_labels.add(label)
            require(commands[label]['argv'] == [exe, str(output / 'requests' / (case.name + '.bin'))], 'direct invocation')
            g.validate(case, raw[label, 'stdout'])
            require(raw[label, 'stdout'] == frozen[case.name], 'frozen complete direct observation')
            if mode == 'sanitized':
                require(raw[label, 'stdout'] == raw['normal-case-' + case.name, 'stdout'], 'direct cross-build equality')
        for name in negatives:
            label = mode + '-negative-' + name
            expected_labels.add(label)
            require(commands[label]['argv'] == [exe, str(output / 'requests' / ('negative-' + name + '.bin'))],
                    'negative invocation')
    def semantic(rows):
        return [{k: v for k, v in r.items() if k != 'peak_guarded_bytes'} for r in rows]
    require(g.exact(semantic(salt_rows['normal']), semantic(salt_rows['sanitized'])), 'salt/state equality')
    peaks = {mode: [r['peak_guarded_bytes'] for r in rows[1:3]] for mode, rows in salt_rows.items()}
    require(g.exact(e['context_peak_guarded_bytes'], peaks), 'mode-qualified memory observations')
    require(set(commands) == expected_labels, 'complete command inventory')
    require({p.name for p in (root / 'logs').iterdir()} == {PurePosixPath(n).name for n in artifacts}, 'log directory inventory')
    require({p.name for p in (root / 'requests').iterdir()} == {n + '.bin' for n in packets}, 'request directory inventory')
    totals = dict(recipes=137, direct_observations=548, fresh_contexts=4, negative_rejections=106)
    require(g.exact(e['totals'], totals), 'capture totals')
    return dict(commands=444, **totals, context_peak_guarded_bytes=peaks)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path, nargs='?')
    parser.add_argument('--check-fixture', action='store_true')
    args = parser.parse_args()
    try:
        require(args.check_fixture != (args.directory is not None), 'choose a capture directory or --check-fixture')
        if args.check_fixture:
            accepted, _ = fixture()
            result = {'frozen_observations': len(accepted['records']), 'pack_bytes': accepted['pack_bytes']}
        else:
            result = audit(args.directory)
    except (ValueError, OSError, TypeError, KeyError) as error:
        parser.exit(1, 'Greedy capture verification failed: ' + str(error) + '\n')
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
