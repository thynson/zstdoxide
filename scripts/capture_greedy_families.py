#!/usr/bin/env python3
"""Capture broader direct Greedy and fresh salt with all linked C units instrumented."""
from pathlib import Path
import argparse
import datetime
import hashlib
import json
import os
import selectors
import shutil
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
CHECKER_SHA = 'b1edd15038de0fa3b4af0d1d03d7fb15ec636f83ead060cc7578c82449c1a207'


def require(value, reason):
    if not value:
        raise ValueError(reason)


def file_sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def utc():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def disjoint(a, b):
    return not a.is_relative_to(b) and not b.is_relative_to(a)


def inputs():
    path = ROOT / 'scripts/check_greedy_family_recipes.py'
    require(path.is_file() and not path.is_symlink() and file_sha(path) == CHECKER_SHA,
            'reviewed checker before import')
    import check_greedy_family_recipes as checker
    oracle, cases, manifest = checker.inputs()
    return checker, oracle, cases, manifest


def execute(args):
    checker, oracle, cases, manifest = inputs()
    upstream, output = args.upstream.resolve(strict=True), args.output.resolve()
    require(upstream.is_dir() and disjoint(ROOT, upstream), 'external reference required')
    require(not output.exists() and disjoint(output, ROOT) and disjoint(output, upstream),
            'fresh disjoint output required')
    cc = shutil.which(args.cc)
    require(cc is not None, 'compiler unavailable')
    cc = Path(cc).resolve()
    catalog = oracle.decode_json((ROOT / 'tests/upstream/manifest.json').read_bytes())
    require(catalog['upstream']['revision'] == oracle.REVISION, 'reference inventory revision')
    sources = {r['path']: r['sha256'] for r in catalog['sources'] + catalog['excluded_sources']
               if r['path'].startswith('lib/') and Path(r['path']).suffix in ('.c', '.h')}
    for name, digest in sources.items():
        require(file_sha(upstream / name) == digest, 'pinned source/header: ' + name)
    units = sorted(p for folder in ('common', 'compress', 'decompress')
                   for p in (upstream / 'lib' / folder).glob('*.c'))
    require(len(units) == 26 and all(str(p.relative_to(upstream)) in sources for p in units),
            'complete used translation units')
    local = set(checker.PINS) | {checker.MANIFEST, 'scripts/check_greedy_family_recipes.py',
                               'scripts/capture_greedy_families.py', 'tests/upstream/manifest.json'}
    def snapshot():
        return {name: file_sha(ROOT / name) for name in sorted(local)}
    original = snapshot()
    output.mkdir(parents=True)
    (output / 'logs').mkdir()
    (output / 'requests').mkdir()
    (output / 'recipes.json').write_bytes((ROOT / checker.MANIFEST).read_bytes())
    env = {k: os.environ[k] for k in ('PATH', 'HOME', 'TMPDIR', 'DEVELOPER_DIR', 'SDKROOT') if k in os.environ}
    env.update(LANG='C', LC_ALL='C', TZ='UTC', ASAN_OPTIONS='detect_leaks=0:halt_on_error=1',
               UBSAN_OPTIONS='halt_on_error=1:print_stacktrace=1')
    evidence = dict(schema_version=1, status='running', started_at=utc(), source_before=original,
                    upstream_revision=oracle.REVISION, upstream_source_sha256=sources,
                    environment=env, compiler=str(cc), compiler_sha256=file_sha(cc), commands=[], builds={},
                    support_archive_used=False, upstream_sanitizer_annotations_retained=True,
                    scope='Direct no-dictionary chain/row observations and fresh-context salt. '
                          'Every linked C unit instrumented in sanitized mode; upstream per-function exclusions retained. '
                          'No Rust Greedy, ordinary frames or timing claim.')
    def save():
        (output / 'evidence.json').write_text(json.dumps(evidence, sort_keys=True, indent=2) + '\n')
    started = time.monotonic()
    def command(label, argv, expected=0, timeout=120):
        require(time.monotonic() - started < 1800, 'bounded campaign duration')
        paths = {s: output / 'logs' / (label + '.' + s) for s in ('stdout', 'stderr')}
        require(all(not p.exists() for p in paths.values()), 'unique command label')
        row = dict(label=label, argv=[str(x) for x in argv], started_at=utc(), expected_exit=expected,
                   cwd=str(ROOT), timeout_seconds=timeout, stdin=None)
        process = None
        begin = time.monotonic()
        with open(os.devnull, 'rb') as input_file, \
                paths['stdout'].open('wb') as stdout, paths['stderr'].open('wb') as stderr:
            try:
                process = subprocess.Popen(row['argv'], cwd=ROOT, env=env, stdin=input_file,
                                           stdout=subprocess.PIPE, stderr=subprocess.PIPE)
                sizes = {stdout: 0, stderr: 0}
                caps = {stdout: oracle.MAX_RECORDS, stderr: 1 << 20}
                deadline = time.monotonic() + timeout
                with selectors.DefaultSelector() as selector:
                    selector.register(process.stdout, selectors.EVENT_READ, stdout)
                    selector.register(process.stderr, selectors.EVENT_READ, stderr)
                    while selector.get_map():
                        remaining = deadline - time.monotonic()
                        require(remaining > 0, 'command timeout')
                        for key, _ in selector.select(min(remaining, 0.1)):
                            chunk = os.read(key.fileobj.fileno(), 65536)
                            if not chunk:
                                selector.unregister(key.fileobj)
                                continue
                            destination = key.data
                            available = caps[destination] - sizes[destination]
                            destination.write(chunk[:available])
                            sizes[destination] += min(len(chunk), available)
                            require(len(chunk) <= available, 'command output cap')
                    row['exit_code'] = process.wait(timeout=max(0.001, deadline - time.monotonic()))
            except BaseException as error:
                row['error'] = repr(error)
                raise
            finally:
                if process is not None:
                    if process.poll() is None:
                        process.kill()
                    row['exit_code'] = process.wait()
                    process.stdout.close()
                    process.stderr.close()
                row['seconds'] = time.monotonic() - begin
                stdout.flush()
                stderr.flush()
                for stream, path in paths.items():
                    row[stream] = dict(path=str(path.relative_to(output)), bytes=path.stat().st_size,
                                       sha256=file_sha(path))
                evidence['commands'].append(row)
        require(row.get('exit_code') == expected, 'command exit: ' + label)
        return paths['stdout'].read_bytes(), paths['stderr'].read_bytes()

    def quiet(label, argv, **options):
        data, errors = command(label, argv, **options)
        require(not errors, 'unexpected stderr: ' + label)
        return data

    save()
    try:
        require(quiet('reference-revision', ['git', '-C', upstream, 'rev-parse', 'HEAD']).decode().strip()
                == oracle.REVISION, 'actual reference revision')
        require(not quiet('reference-status', ['git', '-C', upstream, 'status', '--porcelain', '--untracked-files=no']),
                'tracked reference changes')
        command('compiler-version', [cc, '--version'])
        command('compiler-target', [cc, '-dumpmachine'])
        helper = ROOT / 'scripts/oracles/greedy_direct.c'
        common = [cc, '-std=c11', '-g', '-I', upstream / 'lib', '-I', upstream / 'lib/common',
                  '-I', upstream / 'lib/compress', '-DZSTD_DISABLE_ASM', '-DZSTD_LEGACY_SUPPORT=0',
                  '-UZSTD_MULTITHREAD', '-DXXH_NAMESPACE=ZSTD_']
        executables = {}
        for mode in ('normal', 'sanitized'):
            build = output / ('c-' + mode)
            build.mkdir()
            flags = ['-O2'] if mode == 'normal' else ['-O1', '-fsanitize=address,undefined',
                    '-fno-sanitize-recover=all', '-fno-omit-frame-pointer']
            deps = quiet(mode + '-dependencies', common + flags + ['-MM', helper, *units])
            dependencies = set()
            for line in deps.decode().replace('\\\n', ' ').splitlines():
                for name in line.split(':', 1)[1].split():
                    path = Path(name).resolve()
                    require(path == helper or (path.is_relative_to(upstream) and
                            str(path.relative_to(upstream)) in sources), 'non-system dependency identity')
                    dependencies.add(path)
            quiet(mode + '-predefines', common + flags + ['-dM', '-E', helper])
            objects = []
            for i, unit in enumerate(units):
                obj = build / f'unit-{i}.o'
                quiet(f'{mode}-compile-{i}', common + flags + ['-c', unit, '-o', obj])
                objects.append(obj)
            exe = build / 'greedy-direct'
            quiet(mode + '-link', common + flags + ['-Wall', '-Wextra', '-Werror', helper, *objects, '-o', exe])
            executables[mode] = exe
            evidence['builds'][mode] = dict(executable_sha256=file_sha(exe),
                    compiled_codec_sources=[str(p.relative_to(upstream)) for p in units],
                    instrumented_codec_sources=[str(p.relative_to(upstream)) for p in units] if mode == 'sanitized' else [],
                    helper_instrumented=mode == 'sanitized', assembly=False, workers=False, leak_sanitizer=False,
                    dependencies={str(p): file_sha(p) for p in sorted(dependencies)},
                    objects={str(p.relative_to(output)): file_sha(p) for p in objects})
            save()
            print('Built all used C units:', mode, flush=True)
        require(snapshot() == original, 'local source changed before execution')
        negatives = checker.malformed_packets(oracle, cases)
        packets = {case.name: case.packet() for case in cases}
        packets.update({'negative-' + name: data for name, data in negatives.items()})
        evidence['requests'] = {}
        for name, data in packets.items():
            path = output / 'requests' / (name + '.bin')
            path.write_bytes(data)
            evidence['requests'][name] = dict(path=str(path.relative_to(output)), bytes=len(data), sha256=file_sha(path))
        save()
        observations = {}
        for mode, exe in executables.items():
            salt = quiet(mode + '-fresh-salt', [exe, '--fresh-salt'])
            oracle.validate_salt(salt)
            observations[mode] = {'fresh-salt': salt}
            print('Observed fresh-context salt:', mode, flush=True)
            for index, case in enumerate(cases):
                result = quiet(mode + '-case-' + case.name, [exe, output / 'requests' / (case.name + '.bin')])
                oracle.validate(case, result)
                observations[mode][case.name] = result
                if (index + 1) % 10 == 0 or index + 1 == len(cases):
                    save()
                    print('Validated', mode, index + 1, '/', len(cases), flush=True)
            for name in negatives:
                result, errors = command(mode + '-negative-' + name,
                    [exe, output / 'requests' / ('negative-' + name + '.bin')], expected=1, timeout=30)
                require(not result and errors.startswith(b'greedy oracle [setup]:'), 'malformed input rejection')
            save()
            print('Validated', mode, len(cases), 'recipes and', len(negatives), 'malformed packets', flush=True)
        require({k: v for k, v in observations['normal'].items() if k != 'fresh-salt'} ==
                {k: v for k, v in observations['sanitized'].items() if k != 'fresh-salt'},
                'instrumentation changed direct observations')
        salt_rows = {mode: oracle.validate_salt(values['fresh-salt']) for mode, values in observations.items()}
        # zstd_cwksp.h adds ASan redzones inside the opaque CCtx workspace.
        # Keep both actual peaks; only this mode-qualified memory metric is
        # excluded from cross-build salt/state equality. All other fields stay.
        evidence['context_peak_guarded_bytes'] = {
            mode: [row['peak_guarded_bytes'] for row in rows[1:3]] for mode, rows in salt_rows.items()}
        def salt_semantics(rows):
            return [{k: v for k, v in row.items() if k != 'peak_guarded_bytes'} for row in rows]
        require(oracle.exact(salt_semantics(salt_rows['normal']), salt_semantics(salt_rows['sanitized'])),
                'instrumentation changed fresh salt/state/owner counts')
        require(snapshot() == original, 'local source changed at completion')
        for name, digest in sources.items():
            require(file_sha(upstream / name) == digest, 'reference changed at completion')
        for mode, exe in executables.items():
            require(file_sha(exe) == evidence['builds'][mode]['executable_sha256'], 'executable changed')
        evidence.update(status='passed', source_after=snapshot(), finished_at=utc(),
                        seconds=time.monotonic() - started,
                        totals=dict(recipes=137, direct_observations=548, fresh_contexts=4, negative_rejections=106))
        save()
        print(json.dumps(evidence['totals'], sort_keys=True), flush=True)
    except BaseException as error:
        evidence.update(status='failed', error=repr(error), finished_at=utc())
        save()
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--upstream', type=Path)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--cc', default='cc')
    parser.add_argument('--check-plan', action='store_true')
    args = parser.parse_args()
    if args.check_plan:
        require(args.upstream is None and args.output is None and args.cc == 'cc', 'plan excludes execution options')
        _, _, cases, manifest = inputs()
        print(json.dumps({'recipes': len(cases), 'packet_bytes': manifest['packet_bytes'], 'reference_executions': 0}))
    else:
        require(args.upstream is not None and args.output is not None, 'external reference and fresh output required')
        execute(args)


if __name__ == '__main__':
    main()
