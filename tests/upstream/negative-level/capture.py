#!/usr/bin/env python3
"""Bounded extracted negative-level source-state oracle; never runs fuzzer main."""
from pathlib import Path
import argparse
import datetime
import hashlib
import json
import os
import shutil
import subprocess
import sys

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
REVISION = 'f8745da6ff1ad1e7bab384bd1f9d742439278e99'
FUZZER_SHA = 'dedcfca725c18810ccb8fce1d2b9995f14f8fb879c8b277158a8316ac2cf690c'
DATAGEN_SHA = 'fdc1e21f374f7ce3d45f26702583ed2c606f9b5b7124c673bb3a4cbb09a62397'
TOOLCHAIN = 'nightly-2026-10-08'
STAGES = ('restored', 'initial-match', 'flat-dictionary', 'bad-magic')
FULL = 5 << 20
PREFIX = 1 << 20
DESTINATION = 5263360
SPANS = {
    'CHECK_Z': (123, 129), 'DEFAULT_PROBABILITY': (54, 54),
    'SOURCE_SIZE': (652, 652), 'DESTINATION_SIZE': (654, 654),
    'FULL_RESTORE': (1541, 1541), 'INITIAL_MATCH': (1639, 1642),
    'FAMILY_GATE': (2521, 2528), 'FLAT_CONSTANTS': (2532, 2534),
    'FLAT_RESTORE': (2556, 2556), 'BAD_MAGIC': (3447, 3447), 'TARGET': (3468, 3489),
}


def require(ok, reason):
    if not ok:
        raise ValueError(reason)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def exact(left, right):
    return json.dumps(left, sort_keys=True, separators=(',', ':')) == json.dumps(right, sort_keys=True, separators=(',', ':'))


def decode_json(text):
    def pairs(rows):
        out = {}
        for key, value in rows:
            require(key not in out, 'duplicate JSON key')
            out[key] = value
        return out
    return json.loads(text, object_pairs_hook=pairs,
                      parse_constant=lambda value: (_ for _ in ()).throw(ValueError(value)))


def save(path, data):
    path.write_text(json.dumps(data, sort_keys=True, indent=2) + '\n')


def stamp():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def snapshot(paths):
    return [{'path': str(path), 'size': path.stat().st_size, 'sha256': sha(path)} for path in paths]


def disjoint(left, right):
    require(not left.is_relative_to(right) and not right.is_relative_to(left), 'protected/output path overlap')


def extract(upstream, output):
    fuzzer = upstream / 'tests/fuzzer.c'
    require(sha(fuzzer) == FUZZER_SHA and sha(upstream / 'programs/datagen.c') == DATAGEN_SHA, 'pinned last-writer sources')
    lines = fuzzer.read_text().splitlines(keepends=True)
    records = []
    template = (HERE / 'oracle.c.in').read_text()
    for name, (first, last) in SPANS.items():
        span = ''.join(lines[first - 1:last])
        require(template.count('@' + name + '@') == 1, 'exact template placeholder')
        records.append({'name': name, 'source': 'tests/fuzzer.c', 'line_start': first, 'line_end': last,
                        'sha256': hashlib.sha256(span.encode()).hexdigest(), 'text': span})
        if name == 'TARGET':
            simple = '            if (ZSTD_isError(cSize_1pass)) goto _output_error;\n'
            advanced = '                if (compressionResult != cSize_1pass) goto _output_error;\n'
            require(span.count(simple) == span.count(advanced) == 1, 'unique target capture anchors')
            span = span.replace(simple, simple + '            save_frame("simple", compressedBuffer, cSize_1pass, CNBuffer, srcSize, decodedBuffer, NULL);\n')
            span = span.replace(advanced, advanced + '                save_frame("advanced", compressedBuffer, compressionResult, CNBuffer, srcSize, decodedBuffer, cctx);\n')
        template = template.replace('@' + name + '@', span.rstrip('\n'))
    require('@' not in template, 'all extraction placeholders replaced')
    (output / 'oracle.c').write_text(template)
    data = {'schema_version': 1, 'upstream_revision': REVISION, 'fuzzer_sha256': FUZZER_SHA,
            'datagen_sha256': DATAGEN_SHA, 'spans': records, 'generated_sha256': sha(output / 'oracle.c'),
            'source_state_only': True, 'full_prefix_executed': False,
            'target_edit': 'Insert capture/decode hooks after the original successful simple call and original advanced equal-size check; all original target statements retained.'}
    save(output / 'extraction.json', data)
    return data


def fingerprint(data):
    h = 14695981039346656037
    for byte in data:
        h = ((h ^ byte) * 1099511628211) & 0xffffffffffffffff
    return f'{h:016x}'


def read_exact(path, size):
    require(path.is_file() and path.stat().st_size == size, 'exact bounded file size: ' + str(path))
    return path.read_bytes()


def frame_blocks(encoded):
    require(len(encoded) >= 6 and encoded[:4] == b'\x28\xb5\x2f\xfd', 'standard frame magic')
    descriptor = encoded[4]
    require(descriptor & 0x18 == 0, 'reserved frame bits')
    single = bool(descriptor & 0x20)
    cursor = 5
    if not single:
        window = encoded[cursor]
        cursor += 1
        base = 1 << (10 + (window >> 3))
        window_size = base + (base >> 3) * (window & 7)
    dict_width = (0, 1, 2, 4)[descriptor & 3]
    dictionary = int.from_bytes(encoded[cursor:cursor + dict_width], 'little')
    cursor += dict_width
    fcs_width = (1 if single else 0, 2, 4, 8)[descriptor >> 6]
    require(fcs_width > 0 and cursor + fcs_width <= len(encoded), 'bounded known frame size')
    content = int.from_bytes(encoded[cursor:cursor + fcs_width], 'little') + (256 if fcs_width == 2 else 0)
    cursor += fcs_width
    if single:
        window_size = content
    header_size = cursor
    rows = []
    while True:
        require(cursor + 3 <= len(encoded) and len(rows) < 16, 'bounded complete block header')
        value = int.from_bytes(encoded[cursor:cursor + 3], 'little')
        cursor += 3
        last, kind, size = value & 1, (value >> 1) & 3, value >> 3
        require(kind != 3 and size <= 131072, 'bounded valid block')
        payload = 1 if kind == 1 else size
        require(cursor + payload <= len(encoded), 'bounded complete block payload')
        rows.append({'last': last, 'type': kind, 'payload_size': payload, 'declared_size': size})
        cursor += payload
        if last:
            break
    checksum = bool(descriptor & 4)
    cursor += 4 if checksum else 0
    require(cursor == len(encoded), 'exact frame end')
    return {'content_size': content, 'window_size': window_size, 'header_size': header_size,
            'dictionary_id': dictionary, 'checksum': int(checksum), 'blocks': rows}


def validate(directory, stdout, require_stages=True):
    require(stdout.stat().st_size < 16384, 'bounded observations')
    rows = [decode_json(line) for line in stdout.read_text().splitlines()]
    require(len(rows) == 8 and all(type(row) is dict for row in rows), 'exact observation count')
    metadata = rows[0]
    expected_meta = {'record': 'metadata', 'revision': REVISION, 'version': 10507,
                     'size_t_bytes': metadata.get('size_t_bytes'), 'seed': 0, 'probability_percent': 50,
                     'all_families_gate': True, 'full_prefix_executed': False, 'source_bytes': FULL,
                     'input_bytes': PREFIX, 'destination_capacity': DESTINATION, 'dictionary_bytes': 0, 'level': -1}
    require(type(metadata.get('size_t_bytes')) is int and metadata['size_t_bytes'] in (4, 8)
            and exact(metadata, expected_meta), 'exact metadata/settings')
    states = []
    for name, row in zip(STAGES, rows[1:5]):
        require(set(row) == {'record', 'name', 'size', 'fnv1a64', 'xxh64', 'prefix_fnv1a64', 'prefix_xxh64'}, 'stage schema')
        require(row['record'] == 'stage' and row['name'] == name and type(row['size']) is int and row['size'] == FULL, 'stage identity')
        for field in ('fnv1a64', 'xxh64', 'prefix_fnv1a64', 'prefix_xxh64'):
            require(type(row[field]) is str and len(row[field]) == 16 and all(c in '0123456789abcdef' for c in row[field]), 'stage digest schema')
        if require_stages:
            data = read_exact(directory / (name + '.raw'), FULL)
            require(fingerprint(data) == row['fnv1a64'] and fingerprint(data[:PREFIX]) == row['prefix_fnv1a64'], 'independent stage fingerprints')
            states.append({'name': name, 'size': FULL, 'sha256': hashlib.sha256(data).hexdigest(),
                           'prefix_sha256': hashlib.sha256(data[:PREFIX]).hexdigest()})
    source = read_exact(directory / 'source.raw', PREFIX)
    require(source[0] == 1 and fingerprint(source) == rows[4]['prefix_fnv1a64'], 'bad-magic source prefix')
    if require_stages:
        restored = read_exact(directory / 'restored.raw', FULL)
        initial = read_exact(directory / 'initial-match.raw', FULL)
        flat = read_exact(directory / 'flat-dictionary.raw', FULL)
        bad = read_exact(directory / 'bad-magic.raw', FULL)
        require(initial[:30] == bytes(range(20)) + bytes(range(10)) and initial[30:] == restored[30:], 'exact first-match write range')
        require(flat[31744:] == initial[31744:], 'exact flat regeneration range')
        require(bad[0] == 1 and bad[1:] == flat[1:] and bad[:PREFIX] == source, 'exact last writer and prefix')
    frames = []
    fields = {'record', 'name', 'size', 'fnv1a64', 'xxh64', 'content_size', 'window_size',
              'block_size_max', 'header_size', 'dictionary_id', 'checksum', 'decoded_bytes', 'query_parameters',
              'requested_level', 'applied_parameters', 'applied_frame_parameters', 'workers', 'guards_intact', 'decoded_equal'}
    for name, row in zip(('simple', 'advanced'), rows[5:7]):
        require(set(row) == fields and row['record'] == 'frame' and row['name'] == name, 'exact frame schema')
        require(type(row['size']) is int and 0 < row['size'] <= 1052672, 'bounded frame bytes')
        data = read_exact(directory / (name + '.zst'), row['size'])
        wire = frame_blocks(data)
        for field in ('fnv1a64', 'xxh64'):
            require(type(row[field]) is str and len(row[field]) == 16 and all(c in '0123456789abcdef' for c in row[field]), 'frame digest schema')
        require(fingerprint(data) == row['fnv1a64'], 'independent frame fingerprint')
        require(all(type(row[k]) is int for k in ('content_size', 'window_size', 'block_size_max', 'header_size', 'dictionary_id', 'checksum', 'decoded_bytes', 'requested_level')), 'frame integer types')
        require(all(row[k] == wire[k] for k in ('content_size', 'window_size', 'header_size', 'dictionary_id', 'checksum')), 'independent header observation')
        require(row['content_size'] == row['decoded_bytes'] == PREFIX and row['window_size'] == 524288
                and row['dictionary_id'] == row['checksum'] == 0 and row['block_size_max'] == 131072,
                'reference whole-frame parameters')
        require(row['requested_level'] == -1 and exact(row['query_parameters'], [19, 12, 13, 1, 6, 1, 1]), 'exact queried negative profile')
        require(row['guards_intact'] is True and row['decoded_equal'] is True, 'guard/independent decode checks')
        if name == 'advanced':
            require(exact(row['applied_parameters'], row['query_parameters'])
                    and exact(row['applied_frame_parameters'], [1, 0, 0])
                    and type(row['workers']) is int and row['workers'] == 0, 'actual advanced applied profile')
        else:
            require(row['applied_parameters'] is None and row['applied_frame_parameters'] is None
                    and row['workers'] is None, 'simple private context is not observable')
        frames.append({'name': name, 'size': len(data), 'sha256': hashlib.sha256(data).hexdigest(), **wire})
    require(exact(rows[-1], {'record': 'complete', 'original_size_assertion': True, 'guard_bytes': 64}), 'original assertion completed')
    require(frames[0]['size'] == frames[1]['size'], 'original equal-size assertion')
    return {'metadata': metadata, 'stages': states, 'frames': frames,
            'source_sha256': hashlib.sha256(source).hexdigest(),
            'additional_frame_byte_equality': (directory / 'simple.zst').read_bytes() == (directory / 'advanced.zst').read_bytes()}


def run(args):
    upstream, repo, output = args.upstream.resolve(), args.repo.resolve(), args.output.resolve()
    require(not output.exists(), 'fresh external output directory')
    for path in (upstream, repo, HERE):
        disjoint(output, path)
    require(subprocess.check_output(['git', '-C', str(upstream), 'rev-parse', 'HEAD'], text=True).strip() == REVISION, 'pinned revision')
    require(not subprocess.check_output(['git', '-C', str(upstream), 'status', '--porcelain', '--untracked-files=no'], text=True).strip(), 'clean tracked reference')
    catalog = decode_json((repo / 'tests/upstream/manifest.json').read_text())
    require(catalog['upstream']['revision'] == REVISION, 'source inventory revision')
    known = {row['path']: row['sha256'] for row in catalog['sources'] + catalog['excluded_sources']}
    required = ['tests/fuzzer.c', 'programs/datagen.c', 'programs/datagen.h', 'programs/platform.h', 'lib/compress/clevels.h']
    closure = sorted(p for p in (upstream / 'lib').rglob('*') if p.is_file() and p.suffix in ('.c', '.h', '.S'))
    for path in [*(upstream / name for name in required), *closure]:
        require(known.get(path.relative_to(upstream).as_posix()) == sha(path), 'catalog source identity: ' + str(path))
    output.mkdir(parents=True)
    extraction = extract(upstream, output)
    shutil.copyfile(HERE / 'compare.rs', output / 'compare.rs')
    shutil.copyfile(repo / 'tests/upstream/support/mod.rs', output / 'support.rs')
    sources = [Path(__file__).resolve(), HERE / 'oracle.c.in', HERE / 'compare.rs',
               repo / 'tests/upstream/support/mod.rs', repo / 'rust-toolchain.toml',
               upstream / 'lib/libzstd.a', *(upstream / name for name in required), *closure,
               output / 'oracle.c', output / 'extraction.json', output / 'compare.rs', output / 'support.rs']
    before = snapshot(sources)
    save(output / 'prepared.json', {'schema_version': 1, 'source_before': before, 'extraction': extraction})
    if args.prepare_only:
        print('Prepared extracted C/Rust sources without compiling or executing either.')
        return
    env = {k: v for k, v in os.environ.items() if k in ('PATH', 'HOME', 'TMPDIR', 'TMP', 'TEMP', 'SYSTEMROOT', 'SDKROOT', 'MACOSX_DEPLOYMENT_TARGET', 'DEVELOPER_DIR')}
    env.update({'LC_ALL': 'C', 'LANG': 'C', 'TZ': 'UTC'})
    cc = shutil.which(args.cc, path=env.get('PATH'))
    require(cc is not None, 'C compiler')
    cc = str(Path(cc).resolve())
    evidence = {'schema_version': 1, 'started_at': stamp(), 'status': 'running', 'source_before': before,
                'compiler': cc, 'compiler_sha256': sha(Path(cc)), 'environment': env, 'commands': [],
                'instrumentation': {'helper_datagen_compress_fast': True, 'other_archive_code': False, 'leak_sanitizer': False}}
    def record():
        save(output / 'evidence.json', evidence)
    def command(label, argv, overrides=None):
        require(snapshot(sources) == before, 'source changed before command')
        stdout, stderr = output / (label + '.stdout'), output / (label + '.stderr')
        effective = env | (overrides or {})
        started = stamp()
        exit_code = None
        failure = None
        with stdout.open('w') as out, stderr.open('w') as err:
            try:
                process = subprocess.run(argv, cwd=output, env=effective, stdout=out, stderr=err, timeout=180, check=False)
                exit_code = process.returncode
            except (subprocess.TimeoutExpired, OSError) as error:
                failure = repr(error)
        evidence['commands'].append({'label': label, 'argv': argv, 'cwd': str(output), 'environment': effective,
                                     'started_at': started, 'finished_at': stamp(), 'exit_code': exit_code, 'failure': failure,
                                     'stdout': {'path': stdout.name, 'sha256': sha(stdout)},
                                     'stderr': {'path': stderr.name, 'sha256': sha(stderr)}})
        record()
        require(failure is None and exit_code == 0, 'command failed: ' + label + (': ' + failure if failure else ''))
        require(snapshot(sources) == before, 'source changed during command')
        return stdout
    try:
        record()
        command('compiler-version', [cc, '--version'])
        command('rust-version', ['rustup', 'run', TOOLCHAIN, 'rustc', '-vV'])
        rust_path = Path(command('rust-path', ['rustup', 'which', '--toolchain', TOOLCHAIN, 'rustc']).read_text().strip()).resolve()
        require(rust_path.is_file(), 'actual pinned Rust compiler path')
        evidence['rust_compiler'] = {'path': str(rust_path), 'sha256': sha(rust_path)}
        rust = output / 'compare'
        command('rust-compile', ['rustup', 'run', TOOLCHAIN, 'rustc', '--edition=2021', '-O', '-D', 'warnings', str(output / 'compare.rs'), '-o', str(rust)])
        results = []
        for mode in ('normal', 'sanitized'):
            executable = output / ('oracle-' + mode)
            cmd = [cc, '-std=c11', '-Wall', '-Wextra', '-Werror', '-Wno-deprecated-declarations', '-fno-fast-math']
            for name in ('lib', 'lib/common', 'lib/compress', 'programs'):
                cmd += ['-I', str(upstream / name)]
            cmd += ['-O2'] if mode == 'normal' else ['-O1', '-g', '-fsanitize=address,undefined', '-fno-omit-frame-pointer']
            cmd += [str(output / 'oracle.c'), str(upstream / 'programs/datagen.c')]
            if mode == 'sanitized':
                cmd += [str(upstream / 'lib/compress/zstd_compress.c'), str(upstream / 'lib/compress/zstd_fast.c')]
            cmd += [str(upstream / 'lib/libzstd.a'), '-o', str(executable)]
            command(mode + '-compile', cmd)
            directory = output / mode
            directory.mkdir()
            overrides = {'ASAN_OPTIONS': 'halt_on_error=1:detect_leaks=0', 'UBSAN_OPTIONS': 'halt_on_error=1:print_stacktrace=1'} if mode == 'sanitized' else {}
            stdout = command(mode + '-capture', [str(executable), str(directory)], overrides)
            result = validate(directory, stdout)
            result['mode'] = mode
            comparison = command(mode + '-rust-compare', [str(rust), str(directory)])
            comparison_rows = [decode_json(line) for line in comparison.read_text().splitlines()]
            require(exact(comparison_rows, [{'stage': stage, 'bytes_compared': FULL, 'equal': True} for stage in STAGES] + [{'prefix_bytes_compared': PREFIX, 'equal': True}]), 'all independent Rust comparison stages')
            result['rust_bytes_compared'] = 4 * FULL + PREFIX
            result['artifacts'] = snapshot(sorted(p for p in directory.iterdir() if p.is_file()))
            results.append(result)
            evidence['captures'] = results
            record()
        require((output / 'normal-capture.stdout').read_bytes() == (output / 'sanitized-capture.stdout').read_bytes(), 'normal/sanitized exact observations')
        require(exact({k: v for k, v in results[0].items() if k not in ('mode', 'artifacts')},
                      {k: v for k, v in results[1].items() if k not in ('mode', 'artifacts')}), 'normal/sanitized complete source/frame equality')
        evidence['status'] = 'passed'
    except BaseException as error:
        evidence['status'] = 'failed'
        evidence['error'] = repr(error)
        raise
    finally:
        evidence['finished_at'] = stamp()
        evidence['source_after'] = snapshot(sources)
        record()
    print('Verified four complete 5 MiB source states, the 1 MiB prefix, and both original C frames in normal/sanitized runs.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--upstream', type=Path, required=True)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--cc', default='cc')
    parser.add_argument('--prepare-only', action='store_true')
    run(parser.parse_args())


if __name__ == '__main__':
    main()
