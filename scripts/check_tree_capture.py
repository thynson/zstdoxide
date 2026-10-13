#!/usr/bin/env python3
"""Replay a qualified tree capture offline, including every compile/link command."""
from pathlib import Path, PurePosixPath
import argparse
import hashlib
import json
import os
import re
import math
import datetime
import sys
sys.dont_write_bytecode = True

ROOT = Path(__file__).resolve().parents[1]
CAPTURE_SHA = 'e628bc1e3b8505755be574ca5ed334c452bc26adc5acb6c622a543da10ab9974'
CHECKER_SHA = 'daed6ffef1f1f2c10893b7c9443e9cd16911de4089ef67dd6a758edd2f70ef28'


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


def audit(directory):
    for name, digest in {'scripts/capture_tree_tiny.py': CAPTURE_SHA,
                         'scripts/tree_capture_inputs.py': CHECKER_SHA}.items():
        require(sha(read(ROOT, name, 1 << 20)) == digest, 'reviewed infrastructure identity')
    import tree_capture_inputs as checker
    g, cases, manifest = checker.inputs()
    root = directory.resolve()
    e = g.decode_json(read(root, 'evidence.json', 4 << 20))
    require(e['status'] == 'passed' and g.exact(e['schema_version'], 1) and e['upstream_revision'] == g.REVISION,
            'completed qualified capture')
    require(e['support_archive_used'] is False and e['upstream_sanitizer_annotations_retained'] is True,
            'instrumentation scope')
    local = set(checker.PINS) | {checker.MANIFEST, 'scripts/tree_capture_inputs.py',
                               'scripts/capture_tree_tiny.py', 'tests/upstream/manifest.json'}
    source = {name: sha(read(ROOT, name, 2 << 20)) for name in sorted(local)}
    require(g.exact(e['source_before'], source) and g.exact(e['source_after'], source), 'source closure')
    require(read(root, 'recipes.json', 1 << 20) == read(ROOT, checker.MANIFEST, 1 << 20), 'input manifest bytes')
    inventory = g.decode_json(read(ROOT, 'tests/upstream/manifest.json', 2 << 20))
    c_source = {r['path']: r['sha256'] for r in inventory['sources'] + inventory['excluded_sources']
                if r['path'].startswith('lib/') and Path(r['path']).suffix in ('.c', '.h')}
    units = sorted(n for n in c_source if n.endswith('.c') and PurePosixPath(n).parent.as_posix()
                   in ('lib/common', 'lib/compress', 'lib/decompress'))
    require(len(units) == 26 and g.exact(e['upstream_source_sha256'], c_source), 'complete pinned C inventory')
    units = [n for n in units if n != 'lib/compress/zstd_lazy.c']
    require(len(units) == 25, 'one embedded codec source')
    require(set(e['builds']) == {'normal', 'sanitized'}, 'build inventory')
    require(type(e['environment']) is dict and
            set(e['environment']) <= {'PATH', 'HOME', 'TMPDIR', 'DEVELOPER_DIR', 'SDKROOT',
                'LANG', 'LC_ALL', 'TZ', 'ASAN_OPTIONS', 'UBSAN_OPTIONS'} and
            all(type(k) is str and type(v) is str for k, v in e['environment'].items()) and
            all(e['environment'].get(k) == v for k, v in {'LANG': 'C', 'LC_ALL': 'C', 'TZ': 'UTC'}.items()),
            'explicit bounded capture environment')
    require(e['environment']['ASAN_OPTIONS'] == 'detect_leaks=0:halt_on_error=1' and
            e['environment']['UBSAN_OPTIONS'] == 'halt_on_error=1:print_stacktrace=1', 'sanitizer environment')
    g.integer(len(e['commands']), 170, 170, 'command count')
    commands = {r['label']: r for r in e['commands']}
    require(len(commands) == len(e['commands']), 'unique command labels')
    expected_labels = {'reference-revision', 'reference-status', 'compiler-version', 'compiler-target'}
    artifacts, raw = set(), {}
    for label, command in commands.items():
        require(set(command) == {'label', 'argv', 'started_at', 'expected_exit', 'cwd',
                'timeout_seconds', 'stdin', 'exit_code', 'seconds', 'stdout', 'stderr'}
                and command['stdin'] is None, 'successful bounded file transport')
        require(type(command['seconds']) in (int,float) and math.isfinite(command['seconds']) and
                0 <= command['seconds'] <= command['timeout_seconds'] + 30, 'bounded command duration')
        require(type(command['started_at']) is str and
                datetime.datetime.fromisoformat(command['started_at']).utcoffset() == datetime.timedelta(0), 'UTC command start')
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
            require(not raw[label, 'stdout'] and raw[label, 'stderr'].startswith(b'tree oracle:'),
                    'malformed packet diagnostic')
            require(not any(marker in raw[label, 'stderr'] for marker in
                    (b'AddressSanitizer', b'UndefinedBehaviorSanitizer', b'runtime error:')),
                    'malformed input triggered sanitizer')
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
    helper = original_root / 'scripts/oracles/tree_direct_v1.c'
    common = [cc, '-std=c11', '-g', '-I', str(upstream / 'lib'), '-I', str(upstream / 'lib/common'),
              '-I', str(upstream / 'lib/compress'), '-DZSTD_DISABLE_ASM', '-DZSTD_LEGACY_SUPPORT=0',
              '-UZSTD_MULTITHREAD', '-DXXH_NAMESPACE=ZSTD_']
    for mode in ('normal', 'sanitized'):
        flags = ['-O2'] if mode == 'normal' else ['-O1', '-fsanitize=address,undefined',
                '-fno-sanitize-recover=all', '-fno-omit-frame-pointer']
        build, objects = e['builds'][mode], []
        require(g.exact(build['compiled_codec_sources'], units) and
                g.exact(build['embedded_codec_sources'], ['lib/compress/zstd_lazy.c']) and
                g.exact(build['instrumented_codec_sources'], sorted(units + ['lib/compress/zstd_lazy.c']) if mode == 'sanitized' else []) and
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
        exe = output / ('c-' + mode) / 'tree-direct'
        require(commands[mode + '-link']['argv'] == common + flags +
                ['-Wall', '-Wextra', '-Werror', str(helper), *objects, '-o', str(exe)], 'complete link inputs')
        observed_deps = {}
        for line in raw[mode + '-dependencies', 'stdout'].decode().replace('\\\n', ' ').splitlines():
            for name in line.split(':', 1)[1].split():
                path = Path(os.path.normpath(name))
                require(path == helper or path.is_relative_to(upstream), 'dependency path')
                digest = source['scripts/oracles/tree_direct_v1.c'] if path == helper else c_source[str(path.relative_to(upstream))]
                observed_deps[str(path)] = digest
        require(g.exact(build['dependencies'], observed_deps), 'exact dependency closure')
        expected_objects = {f'c-{mode}/unit-{i}.o' for i in range(25)}
        require(set(build['objects']) == expected_objects and all(re.fullmatch('[0-9a-f]{64}', h)
                for h in build['objects'].values()), 'object hash inventory')
        for name, digest in {**build['objects'], f'c-{mode}/tree-direct': build['executable_sha256']}.items():
            require(re.fullmatch('[0-9a-f]{64}', digest), 'binary identity')
            require(sha(read(root, name, 64 << 20)) == digest, 'required binary hash')
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
    for mode in ('normal', 'sanitized'):
        exe = str(output / ('c-' + mode) / 'tree-direct')
        for case in cases:
            label = mode + '-case-' + case.name
            expected_labels.add(label)
            require(commands[label]['argv'] == [exe, str(output / 'requests' / (case.name + '.bin'))], 'direct invocation')
            g.validate(case, raw[label, 'stdout'])
            if mode == 'sanitized':
                require(raw[label, 'stdout'] == raw['normal-case-' + case.name, 'stdout'], 'complete cross-build equality')
        for name in negatives:
            label = mode + '-negative-' + name
            expected_labels.add(label)
            require(commands[label]['argv'] == [exe, str(output / 'requests' / ('negative-' + name + '.bin'))], 'negative invocation')
    require(set(commands) == expected_labels, 'complete command inventory')
    require({p.name for p in (root / 'logs').iterdir()} == {PurePosixPath(n).name for n in artifacts}, 'log directory inventory')
    require({p.name for p in (root / 'requests').iterdir()} == {n + '.bin' for n in packets}, 'request directory inventory')
    totals = dict(recipes=30, direct_observations=120, operation_records=4 * sum(len(c.request.queries) for c in cases), negative_rejections=50)
    require(g.exact(e['totals'], totals), 'capture totals')
    expected_order = ['reference-revision', 'reference-status', 'compiler-version', 'compiler-target']
    for mode in ('normal', 'sanitized'):
        expected_order += [mode + '-dependencies', mode + '-predefines',
                           *[f'{mode}-compile-{i}' for i in range(25)], mode + '-link']
    for mode in ('normal', 'sanitized'):
        expected_order += [*[mode + '-case-' + c.name for c in cases],
                           *[mode + '-negative-' + n for n in negatives]]
    require([r['label'] for r in e['commands']] == expected_order, 'complete command order')
    return dict(commands=170, **totals)



def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    args = parser.parse_args()
    try:
        result = audit(args.directory)
    except (ValueError, OSError, TypeError, KeyError) as error:
        parser.exit(1, 'Tree capture verification failed: ' + str(error) + '\n')
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
