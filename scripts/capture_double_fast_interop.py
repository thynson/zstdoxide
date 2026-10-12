#!/usr/bin/env python3
"""Capture reviewed public DoubleFast frames with all used C units instrumented."""
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
TOOLCHAIN = 'nightly-2026-10-08'
PINS = {
    'scripts/check_double_fast_interop_recipes.py': '83af1fd5b89b0d512a3a1294492b6b5a9e73ad61f0bb992c043f839da3ae0fcf',
    'scripts/oracles/double_fast_interop.py': '05eefcf492ce7dca0741d3348d165aca8a48e402aa63710d1c043775125705f3',
    'scripts/oracles/double_fast_interop_protocol.py': 'af9562efe67846530ab364ce6c97388af7f407147040b67e330643517e5ad543',
    'scripts/oracles/reference_double_fast_frames.c': 'dca0eb63b4eed76324de5d4ae9134b0f7dc7a5a7868479fa7eb9bec310171b2c',
    'examples/reference_double_fast_oracle.rs': 'ed455584173111e8186b41e7eec16645fa6c267828a0935831e74196ba87895b',
    'docs/reference-double-fast-interop-adapter-review.md': 'ecbca0212280b1dc228f798b62a8a373ba25e4d5cbcc941f7b33befd249b5acc',
}


def require(value, reason):
    if not value:
        raise ValueError(reason)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def file_sha(path):
    return sha(path.read_bytes())


def utc():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def disjoint(left, right):
    return not left.is_relative_to(right) and not right.is_relative_to(left)


def inputs():
    for name, digest in PINS.items():
        path = ROOT / name
        require(path.is_file() and not path.is_symlink() and file_sha(path) == digest,
                'reviewed identity before import: ' + name)
    import check_double_fast_interop_recipes as checker
    from oracles import double_fast_interop as generator, double_fast_interop_protocol as protocol
    manifest = checker.check()
    return checker, generator, protocol, manifest


def execute(args):
    checker, generator, protocol, manifest = inputs()
    cases = generator.recipes()
    upstream, output = args.upstream.resolve(strict=True), args.output.resolve()
    require(upstream.is_dir() and disjoint(ROOT, upstream), 'external reference required')
    require(not output.exists() and all(disjoint(output, p) for p in (ROOT, upstream)),
            'fresh disjoint output required')
    cc = shutil.which(args.cc)
    require(cc is not None, 'C compiler unavailable')
    cc = Path(cc).resolve()
    catalog = checker.decode((ROOT / 'tests/upstream/manifest.json').read_bytes())
    require(catalog['upstream']['revision'] == generator.REVISION, 'pinned inventory revision')
    source_map = {r['path']: r['sha256'] for r in catalog['sources'] + catalog['excluded_sources']
                  if r['path'].startswith('lib/') and Path(r['path']).suffix in ('.c', '.h')}
    for name, digest in source_map.items():
        require(file_sha(upstream / name) == digest, 'pinned C source/header: ' + name)
    c_sources = sorted(p for folder in ('common', 'compress', 'decompress')
                       for p in (upstream / 'lib' / folder).glob('*.c'))
    require(len(c_sources) == 26 and all(str(p.relative_to(upstream)) in source_map for p in c_sources),
            'complete used C translation units')
    local_paths = set(ROOT / name for name in PINS)
    local_paths.update((ROOT / 'src').rglob('*.rs'))
    local_paths.update(ROOT / name for name in ('Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
                       'tests/upstream/manifest.json', 'scripts/capture_double_fast_interop.py',
                       'scripts/check_reference_fast_interop.py', 'scripts/import_reference_fast_fuzz.py'))
    local_paths.update(ROOT / name for name in generator.PINS)
    local_paths.add(checker.MANIFEST)

    def snapshot():
        return {str(p.relative_to(ROOT)): file_sha(p) for p in sorted(local_paths)}

    original = snapshot()
    output.mkdir(parents=True)
    (output / 'logs').mkdir()
    (output / 'requests').mkdir()
    (output / 'recipes.json').write_bytes(checker.MANIFEST.read_bytes())
    env = {key: os.environ[key] for key in ('PATH', 'HOME', 'TMPDIR', 'RUSTUP_HOME', 'CARGO_HOME',
                                          'DEVELOPER_DIR', 'SDKROOT') if key in os.environ}
    env.update(LANG='C', LC_ALL='C', TZ='UTC', CARGO_NET_OFFLINE='true', CARGO_TERM_COLOR='never',
               RUSTUP_TOOLCHAIN=TOOLCHAIN, ASAN_OPTIONS='detect_leaks=0:halt_on_error=1',
               UBSAN_OPTIONS='halt_on_error=1:print_stacktrace=1')
    evidence = dict(schema_version=1, status='running', started_at=utc(), source_before=original,
                    upstream_revision=generator.REVISION, upstream_source_sha256=source_map,
                    environment=env, compiler=str(cc), compiler_sha256=file_sha(cc), commands=[], cases=[],
                    builds={}, support_archive_used=False,
                    scope='Public known-size request frames. Safe/all Rust equality, bidirectional decoded bytes, '
                          'normal/full-codec ASan/UBSan equality. No C applied-scalar, entropy-identity or timing claim.')

    def save():
        (output / 'evidence.json').write_text(json.dumps(evidence, sort_keys=True, indent=2) + '\n')

    started = time.monotonic()

    def command(label, argv, data=None, expected=0, timeout=180):
        require(time.monotonic() - started < 1800, 'bounded campaign duration')
        paths = {s: output / 'logs' / (label + '.' + s) for s in ('stdout', 'stderr')}
        require(all(not p.exists() for p in paths.values()), 'unique command label')
        row = dict(label=label, argv=[str(x) for x in argv], started_at=utc(), expected_exit=expected,
                   cwd=str(ROOT), timeout_seconds=timeout, stdin=None)
        stdin_path = None
        if data is not None:
            require(type(data) is bytes and len(data) <= protocol.MAX_REQUEST + 1, 'bounded request bytes')
            stdin_path = output / 'requests' / (label + '.bin')
            require(not stdin_path.exists(), 'new request path')
            stdin_path.write_bytes(data)
            row['stdin'] = dict(path=str(stdin_path.relative_to(output)), bytes=len(data), sha256=sha(data))
        process = None
        begin = time.monotonic()
        with open(os.devnull if stdin_path is None else stdin_path, 'rb') as input_file, \
                paths['stdout'].open('wb') as stdout, paths['stderr'].open('wb') as stderr:
            try:
                process = subprocess.Popen(row['argv'], cwd=ROOT, env=env, stdin=input_file,
                                           stdout=subprocess.PIPE, stderr=subprocess.PIPE)
                sizes = {stdout: 0, stderr: 0}
                caps = {stdout: protocol.MAX_RESPONSE, stderr: 2 << 20}
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
                == generator.REVISION, 'actual reference revision')
        require(not quiet('reference-status', ['git', '-C', upstream, 'status', '--porcelain', '--untracked-files=no']),
                'tracked reference changes')
        command('compiler-version', [cc, '--version'])
        command('compiler-target', [cc, '-dumpmachine'])
        command('rustc-version', ['rustc', '+' + TOOLCHAIN, '-vV'])
        compiler = quiet('rustc-path', ['rustup', 'which', '--toolchain', TOOLCHAIN, 'rustc']).decode().strip()
        evidence['rustc_sha256'] = file_sha(Path(compiler))
        helper = ROOT / 'scripts/oracles/reference_double_fast_frames.c'
        common = [cc, '-std=c11', '-g', '-I', upstream / 'lib', '-I', upstream / 'lib/common',
                  '-I', upstream / 'lib/compress', '-DZSTD_DISABLE_ASM', '-DZSTD_LEGACY_SUPPORT=0',
                  '-DXXH_NAMESPACE=ZSTD_']
        c_executables, rust_executables = {}, {}
        for mode in ('normal', 'sanitized'):
            build = output / ('c-' + mode)
            build.mkdir()
            flags = ['-O2'] if mode == 'normal' else ['-O1', '-fsanitize=address,undefined',
                    '-fno-sanitize-recover=all', '-fno-omit-frame-pointer']
            deps = quiet(mode + '-dependencies', common + flags + ['-MM', helper, *c_sources])
            dependencies = set()
            for line in deps.decode().replace('\\\n', ' ').splitlines():
                for name in line.split(':', 1)[1].split():
                    path = Path(name).resolve()
                    require(path == helper or (path.is_relative_to(upstream) and
                            str(path.relative_to(upstream)) in source_map), 'unexpected non-system dependency')
                    dependencies.add(path)
            command(mode + '-predefines', common + flags + ['-dM', '-E', helper])
            objects = []
            for i, source in enumerate(c_sources):
                obj = build / f'unit-{i}.o'
                quiet(f'{mode}-compile-{i}', common + flags + ['-c', source, '-o', obj])
                objects.append(obj)
            exe = build / 'reference-double-fast'
            quiet(mode + '-link', common + flags + ['-Wall', '-Wextra', '-Werror', helper, *objects, '-o', exe])
            c_executables[mode] = exe
            evidence['builds'][mode] = dict(executable_sha256=file_sha(exe),
                    compiled_codec_sources=[str(p.relative_to(upstream)) for p in c_sources],
                    instrumented_codec_sources=[str(p.relative_to(upstream)) for p in c_sources] if mode == 'sanitized' else [],
                    helper_instrumented=mode == 'sanitized', assembly=False, workers=False, leak_sanitizer=False,
                    dependencies={str(p): file_sha(p) for p in sorted(dependencies)})
            save()
            print('Built C', mode, flush=True)
        for mode, flags in (('safe', []), ('all', ['--all-features'])):
            target = output / ('rust-' + mode)
            command('rust-build-' + mode, ['cargo', '+' + TOOLCHAIN, 'build', '--offline', '--locked', '--release',
                    '--example', 'reference_double_fast_oracle', '--target-dir', target, *flags])
            exe = target / 'release/examples/reference_double_fast_oracle'
            rust_executables[mode] = exe
            evidence['builds'][mode] = dict(executable_sha256=file_sha(exe))
            save()
            print('Built Rust', mode, flush=True)
        require(snapshot() == original, 'source changed during builds')
        for mode, exe in {**c_executables, **rust_executables}.items():
            for name, data in protocol.negative_packets():
                response, errors = command(mode + '-negative-' + name, [exe], data, expected=2, timeout=30)
                require(not response and errors.startswith(b'reference DoubleFast'), 'negative protocol diagnostics')
        evidence['negative_rejections'] = 38 * 4
        save()
        totals = dict(recipes=159, admitted=0, unsupported=0, resource=0,
                      rust_frames_c_decoded=0, c_frames_rust_decoded=0, decoded_bytes=0)
        rust_frames = {}
        for index, case in enumerate(cases):
            require(snapshot() == original, 'source changed before case')
            prefix = f'case-{index:03}'
            row = dict(recipe=case.metadata(), rust={}, c={})
            encoded = {}
            for mode, exe in rust_executables.items():
                reply = quiet(prefix + '-rust-encode-' + mode, [exe], data=protocol.packet(case.source, case.request), timeout=30)
                status, resolved, frame = protocol.response(reply, b'ZDOR0001')
                require(status == {'success': 0, 'unsupported': 1, 'resource': 2}[case.expected], 'expected admission')
                encoded[mode] = (status, resolved, frame)
                row['rust'][mode] = dict(status=case.expected, resolved=resolved, frame_bytes=len(frame), frame_sha256=sha(frame))
                if status == 0:
                    info = protocol.frame_info(frame, len(case.source))
                    require(info['window_size'] <= 1 << resolved[0], 'resolved Rust window')
                    row['rust'][mode]['frame'] = info
            require(encoded['safe'] == encoded['all'], 'safe/all Rust frames and scalar observations')
            if case.expected != 'success':
                totals[case.expected] += 1
                row['c']['status'] = 'not-run-for-rejected-Rust-profile'
            else:
                totals['admitted'] += 1
                rust_frames[case.name] = encoded['safe'][2]
                c_frames = []
                for mode, exe in c_executables.items():
                    for rust_mode, (_, _, frame) in encoded.items():
                        reply = quiet(prefix + '-c-' + mode + '-decode-' + rust_mode, [exe],
                                      data=protocol.packet(case.source, case.request, frame, 1), timeout=30)
                        status, zeros, decoded = protocol.response(reply, b'ZDOC0001')
                        require(status == 0 and zeros == [0] * 7 and decoded == case.source, 'C decoded original Rust input')
                        totals['rust_frames_c_decoded'] += 1
                        totals['decoded_bytes'] += len(decoded)
                    reply = quiet(prefix + '-c-encode-' + mode, [exe],
                                  data=protocol.packet(case.source, case.request), timeout=30)
                    status, zeros, frame = protocol.response(reply, b'ZDOC0001')
                    require(status == 0 and zeros == [0] * 7, 'C encode response')
                    row['c'][mode] = dict(frame_bytes=len(frame), frame_sha256=sha(frame),
                                         frame=protocol.frame_info(frame, len(case.source)))
                    c_frames.append(frame)
                    for rust_mode, rust_exe in rust_executables.items():
                        reply = quiet(prefix + '-rust-' + rust_mode + '-decode-' + mode, [rust_exe],
                                      data=protocol.packet(case.source, case.request, frame, 1), timeout=30)
                        status, zeros, decoded = protocol.response(reply, b'ZDOR0001')
                        require(status == 0 and zeros == [0] * 7 and decoded == case.source, 'Rust decoded original C input')
                        totals['c_frames_rust_decoded'] += 1
                        totals['decoded_bytes'] += len(decoded)
                require(c_frames[0] == c_frames[1], 'C instrumentation changed frame bytes')
            evidence['cases'].append(row)
            evidence['totals'] = totals
            save()
            print(index + 1, '/', len(cases), case.name, case.expected, flush=True)
        for group in manifest['expected_rust_frame_equivalence']:
            require(all(rust_frames[name] == rust_frames[group[0]] for name in group), 'fixed frame-equivalence hypothesis')
        require((totals['admitted'], totals['unsupported'], totals['resource']) == (113, 42, 4), 'complete classifications')
        require(snapshot() == original, 'local source changed at completion')
        for name, digest in source_map.items():
            require(file_sha(upstream / name) == digest, 'reference changed at completion')
        for mode, exe in {**c_executables, **rust_executables}.items():
            require(file_sha(exe) == evidence['builds'][mode]['executable_sha256'], 'executable changed')
        evidence.update(status='passed', source_after=snapshot(), finished_at=utc(), equivalence_groups=16,
                        seconds=time.monotonic() - started)
        save()
        print(json.dumps(totals, sort_keys=True), flush=True)
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
        require(args.upstream is None and args.output is None and args.cc == 'cc', 'offline plan excludes execution arguments')
        _, _, _, manifest = inputs()
        print(json.dumps({key: manifest[key] for key in ('recipe_count', 'source_bytes', 'classifications')}, sort_keys=True))
    else:
        require(args.upstream is not None and args.output is not None, 'external reference and fresh output required')
        execute(args)


if __name__ == '__main__':
    main()
