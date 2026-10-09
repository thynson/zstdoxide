#!/usr/bin/env python3
"""Validate the 81-case reference-Fast fuzz selector capture without processes.

Default/--check is offline. --upstream and fresh external --output explicitly
regenerate and compare; --sanitize adds a scoped ASan/UBSan comparison. Initial
--capture writes only an absent collection capture after successful validation.
"""
from pathlib import Path
import argparse
import csv
import hashlib
import io
import json
import os
import shlex
import shutil
import struct
import subprocess
import sys
import time

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
COLLECTION = ROOT / 'tests/upstream/reference-fast-fuzz'
GENERATOR = COLLECTION / 'selector_oracle.c'
REVISION = 'f8745da6ff1ad1e7bab384bd1f9d742439278e99'
LIMIT = 1 << 20
COUNT = 81
# Captured with the unchanged pinned C producer before Rust selector assertions.
OBSERVATIONS_SHA256 = '77a868913331f7e9edd4a73a60634eebd97d83b5b57050b6945fbde5b317b4e5'
SOURCES = ('tests/fuzz/simple_round_trip.c', 'tests/fuzz/fuzz_data_producer.c',
           'tests/fuzz/fuzz_data_producer.h', 'tests/fuzz/fuzz_helpers.c',
           'tests/fuzz/fuzz_helpers.h', 'tests/fuzz/zstd_helpers.c', 'lib/zstd.h',
           'lib/common/zstd_common.c', 'lib/compress/zstd_compress.c')
FIELDS = ('case', 'family', 'prefix_recipe_size', 'tail_hex', 'input_sha256',
          'input_size', 'prefix_size', 'remaining_after_prefix', 'compression_bound',
          'capacity_reduction', 'remaining_after_capacity', 'branch',
          'remaining_after_branch', 'level', 'remaining_after_level',
          'decoder_toggle', 'remaining_after_decoder')
POLICY = ('Selector-only C observations. Simple-branch numeric levels are selected '
          'exactly; advanced parameters remain unconsumed after the branch. Captured '
          'size budgets do not establish caller-owned destination, context reuse, '
          'mutable decoder settings or overlapping in-place behavior. Expected '
          'admission counts are Rust policy assertions for the later integration '
          'test, not a C codec run.')


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def bounded(path, maximum):
    require(path.is_file() and not path.is_symlink() and path.stat().st_size <= maximum,
            'bounded regular file required: ' + str(path))
    with path.open('rb') as f:
        data = f.read(maximum + 1)
    require(len(data) <= maximum, 'file grew past cap')
    return data


def decode_json(data):
    def pairs(items):
        value = {}
        for key, entry in items:
            require(key not in value, 'duplicate JSON key')
            value[key] = entry
        return value
    return json.loads(data, object_pairs_hook=pairs,
                      parse_constant=lambda value: (_ for _ in ()).throw(ValueError(value)))


def serialized(value):
    return json.dumps(value, indent=2, sort_keys=True) + '\n'


def exact(left, right):
    return serialized(left) == serialized(right)


def width(value):
    return (value.bit_length() + 7) // 8


def cases():
    result = []
    def controlled(family, size, level, branch, reduction, decoder):
        control = bytes((decoder, level + 3, branch, reduction))
        for reserve_width in range(1, 5):
            total = size + 4 + reserve_width
            if width(total) == reserve_width:
                tail = control + (4).to_bytes(reserve_width, 'little')
                break
        else:
            raise ValueError('no consistent reservation width')
        source = bytes((17 * i + 123) & 255 for i in range(size))
        result.append({'case': f'{family}-{len(result):02}', 'family': family,
                       'prefix_recipe_size': size, 'tail_hex': tail.hex(),
                       'input': source + tail, 'level': level, 'branch': branch,
                       'reduction': reduction, 'decoder': decoder})
    sizes = (0, 1, 30, 250, 251, 65529, 65530, 131072, 131073, 262144, 262145)
    for size in sizes:
        for level in (-3, -2, -1, 1, 2):
            index = len(result)
            controlled('tier', size, level, 0, index & 1, (index >> 1) & 1)
    for level in (0, *range(3, 20)):
        index = len(result)
        controlled('unsupported', 30, level, 0, index & 1, (index >> 1) & 1)
    for raw in (b'', b'\0', b'\1', b'\xff'):
        result.append({'case': f'exhausted-{len(result):02}', 'family': 'exhausted',
                       'prefix_recipe_size': 0, 'tail_hex': raw.hex(), 'input': raw,
                       'level': -3, 'branch': 0, 'reduction': 0, 'decoder': 0})
    for size in (0, 30, 251, 65530):
        index = len(result)
        controlled('advanced', size, -3, 1, index & 1, (index >> 1) & 1)
    require(len(result) == COUNT, 'exact case count')
    require(sum(len(c['input']) for c in result) <= 16 * LIMIT, 'batch cap')
    return result


def packet(items):
    return b'ZRFZIN01' + struct.pack('<I', COUNT) + b''.join(
        struct.pack('<I', len(c['input'])) + c['input'] for c in items)


def make_tsv(items, values):
    out = io.StringIO(newline='')
    writer = csv.writer(out, delimiter='\t', lineterminator='\n')
    writer.writerow(FIELDS)
    for case, row in zip(items, values, strict=True):
        writer.writerow([case['case'], case['family'], case['prefix_recipe_size'],
                         case['tail_hex'], digest(case['input']), *row])
    return out.getvalue().encode('ascii')


def validate(data):
    require(len(data) <= 65536, 'TSV cap')
    reader = csv.DictReader(io.StringIO(data.decode('ascii')), delimiter='\t')
    require(tuple(reader.fieldnames or ()) == FIELDS, 'exact TSV schema')
    rows = list(reader)
    require(len(rows) == COUNT and all(None not in row and None not in row.values() for row in rows), 'exact complete rows')
    totals = {'admitted': 0, 'unsupported': 0, 'advanced': 0}
    items = cases()
    for row, case in zip(rows, items, strict=True):
        for field in ('case', 'family', 'tail_hex'):
            require(row[field] == case[field], 'deterministic case identity')
        require(row['prefix_recipe_size'] == str(case['prefix_recipe_size']), 'recipe extent')
        require(row['input_sha256'] == digest(case['input']), 'complete packet identity')
        numeric = {}
        for key in FIELDS[5:]:
            value = row[key]
            if value != '-':
                require(value == str(int(value)), 'canonical integer')
                numeric[key] = int(value)
        n = len(case['input'])
        require(numeric['input_size'] == n <= LIMIT, 'original input size')
        require(numeric['prefix_size'] == case['prefix_recipe_size'], 'selected exact prefix')
        remaining = 0 if case['family'] == 'exhausted' else 4
        require(numeric['remaining_after_prefix'] == remaining, 'post-prefix remaining')
        require(numeric['capacity_reduction'] == case['reduction'], 'capacity selector')
        require(numeric['remaining_after_capacity'] == max(remaining - 1, 0), 'capacity consumption')
        require(numeric['branch'] == case['branch'], 'branch selector')
        require(numeric['remaining_after_branch'] == max(remaining - 2, 0), 'branch consumption')
        require(n < numeric['compression_bound'] <= n + 65536, 'plausible original C bound')
        # Exact bound bytes are a C observation, not recomputed with a duplicate formula.
        if case['branch']:
            require(all(row[k] == '-' for k in FIELDS[-4:]), 'advanced bytes remain unconsumed')
            totals['advanced'] += 1
        else:
            require(numeric['level'] == case['level'], 'selected signed level')
            require(numeric['remaining_after_level'] == max(remaining - 3, 0), 'level consumption')
            require(numeric['decoder_toggle'] == case['decoder'], 'decoder toggle')
            require(numeric['remaining_after_decoder'] == 0, 'decoder consumption')
            admitted = case['level'] in (-3, -2, -1, 1, 2) and not (
                case['level'] == 2 and 131072 < case['prefix_recipe_size'] <= 262144)
            totals['admitted' if admitted else 'unsupported'] += 1
    require(totals == {'admitted': 57, 'unsupported': 20, 'advanced': 4}, 'explicit classification totals')
    return totals


def unpack(data):
    require(len(data) == 24 + COUNT * 48 and data[:8] == b'ZRFZOUT1', 'fixed response extent/magic')
    count, pointer, integer, version = struct.unpack_from('<4I', data, 8)
    require(count == COUNT and pointer in (32, 64) and integer == 32 and version == 10507, 'observed C host')
    rows = []
    for index in range(COUNT):
        row = list(struct.unpack_from('<12I', data, 24 + index * 48))
        if row[6]:
            require(row[8:] == [0x80000000, 0xffffffff, 0xffffffff, 0xffffffff], 'advanced response sentinel')
            row[8:] = ['-'] * 4
        elif row[8] & 0x80000000:
            row[8] -= 1 << 32
        rows.append(row)
    tsv = make_tsv(cases(), rows)
    validate(tsv)
    return tsv, {'size_t_bits': pointer, 'int_bits': integer, 'version': version}


def catalog():
    value = decode_json(bounded(ROOT / 'tests/upstream/manifest.json', 4 << 20))
    require(value['upstream']['revision'] == REVISION, 'pinned inventory revision')
    return {r['path']: r['sha256'] for r in value['sources'] + value['excluded_sources']}


def offline():
    manifest = decode_json(bounded(COLLECTION / 'manifest.json', 1 << 20))
    require(type(manifest) is dict and manifest['schema_version'] == 1 and type(manifest['schema_version']) is int, 'manifest schema')
    require(manifest['upstream_revision'] == REVISION and manifest['policy'] == POLICY, 'provenance identity')
    require(manifest['generator'] == GENERATOR.relative_to(ROOT).as_posix()
            and manifest['generator_sha256'] == digest(bounded(GENERATOR, 65536)), 'generator identity')
    importer = Path(__file__).resolve()
    require(manifest['importer'] == importer.relative_to(ROOT).as_posix()
            and manifest['importer_sha256'] == digest(bounded(importer, 65536)), 'importer identity')
    pinned = catalog()
    sources = manifest['source_sha256']
    require(type(sources) is dict and set(SOURCES) <= sources.keys(), 'source set')
    require(all(pinned.get(path) == value for path, value in sources.items()), 'pinned source hashes')
    data = bounded(COLLECTION / 'observations.tsv', 65536)
    require(digest(data) == OBSERVATIONS_SHA256, 'independent frozen observation identity')
    require(exact(manifest['fixtures'], [{'path': 'observations.tsv', 'size': len(data), 'sha256': digest(data), 'rows': COUNT}]), 'exact fixture catalog')
    require(set(p.name for p in COLLECTION.iterdir()) == {'README.md', 'manifest.json', 'selector_oracle.c', 'observations.tsv'}, 'exact collection files')
    totals = validate(data)
    require(exact(manifest['expected_rust_classifications'], totals), 'classification metadata')
    host = manifest['capture']['host']
    require(exact(host, {'size_t_bits': host['size_t_bits'], 'int_bits': 32, 'version': 10507})
            and type(host['size_t_bits']) is int and host['size_t_bits'] in (32, 64), 'capture scalar widths')
    return data, manifest


def disjoint(left, right):
    require(not left.is_relative_to(right) and not right.is_relative_to(left), 'protected output overlap')


def regenerate(args):
    upstream, output = args.upstream.resolve(), args.output.resolve()
    require(not output.exists(), 'fresh external output required')
    for protected in (ROOT, upstream):
        disjoint(output, protected)
    if args.capture:
        require(not (COLLECTION / 'observations.tsv').exists() and not (COLLECTION / 'manifest.json').exists(), 'initial capture only; never overwrite')
        expected = None
    else:
        expected, _ = offline()
    output.mkdir(parents=True)
    env = {k: v for k, v in os.environ.items() if k in (
        'PATH', 'HOME', 'TMPDIR', 'TMP', 'TEMP', 'SYSTEMROOT', 'SDKROOT',
        'MACOSX_DEPLOYMENT_TARGET', 'DEVELOPER_DIR')}
    env.update(LC_ALL='C', LANG='C', TZ='UTC')
    records = []
    report = {'status': 'running', 'commands': records, 'environment': env,
              'scope': 'Only helper, unchanged producer/helpers and included header code are sanitized; static codec archive is not instrumented. No codec execution except version and compressBound queries.'}
    def save_report():
        (output / 'evidence.json').write_text(serialized(report))
    def command(label, argv, data=None, overrides=None):
        start = time.monotonic()
        stdout, stderr = output / (label + '.stdout'), output / (label + '.stderr')
        effective = env | (overrides or {})
        status, failure = None, None
        with stdout.open('xb') as out, stderr.open('xb') as err:
            try:
                run = subprocess.run(argv, input=data, cwd=output, env=effective, stdout=out, stderr=err, timeout=120, check=False)
                status = run.returncode
            except (OSError, subprocess.TimeoutExpired) as error:
                failure = repr(error)
        records.append({'label': label, 'argv': list(map(str, argv)), 'cwd': str(output),
                        'environment': effective, 'exit_code': status, 'exception': failure,
                        'seconds': time.monotonic() - start,
                        'stdin_sha256': None if data is None else digest(data),
                        'stdout': {'path': stdout.name, 'size': stdout.stat().st_size, 'sha256': digest(stdout.read_bytes())},
                        'stderr': {'path': stderr.name, 'size': stderr.stat().st_size, 'sha256': digest(stderr.read_bytes())}})
        save_report()
        require(status == 0 and failure is None, 'command failed: ' + label)
        return stdout.read_bytes()
    try:
        require(command('revision', ['git', '-C', str(upstream), 'rev-parse', 'HEAD']).decode().strip() == REVISION, 'pinned checkout')
        require(not command('clean', ['git', '-C', str(upstream), 'status', '--porcelain', '--untracked-files=no']).strip(), 'clean tracked source')
        pinned = catalog()
        closure = [upstream / name for name in SOURCES]
        closure += sorted(p for p in (upstream / 'lib').rglob('*') if p.is_file() and p.suffix in ('.h', '.c', '.S'))
        closure += sorted(p for p in (upstream / 'tests/fuzz').glob('*.h'))
        closure = sorted(set(closure))
        for path in closure:
            require(digest(path.read_bytes()) == pinned.get(path.relative_to(upstream).as_posix()), 'source catalog binding: ' + str(path))
        helper_text = (upstream / 'tests/fuzz/zstd_helpers.c').read_text()
        require('const int kMinClevel = -3;' in helper_text and 'const int kMaxClevel = 19;' in helper_text, 'exact original level bounds')
        library = upstream / 'lib/libzstd.a'
        require(library.is_file(), 'prebuilt external static archive required')
        owned = [Path(__file__).resolve(), GENERATOR, COLLECTION / 'README.md']
        paths = [*closure, library, *owned]
        before = {str(path): digest(path.read_bytes()) for path in paths}
        report['input_sha256'] = before
        cc = shutil.which(args.cc or 'cc', path=env.get('PATH'))
        require(cc is not None, 'compiler executable')
        cc = str(Path(cc).resolve())
        compiler_version = command('compiler-version', [cc, '--version']).decode()
        report['compiler'] = {'path': cc, 'sha256': digest(Path(cc).read_bytes()), 'version': compiler_version}
        input_data = packet(cases())
        (output / 'request.bin').write_bytes(input_data)
        observed = []
        dependencies = set(SOURCES)
        modes = ('normal', 'sanitized') if args.sanitize else ('normal',)
        for mode in modes:
            flags = ['-std=c11', '-Wall', '-Wextra', '-Werror', '-Wno-deprecated-declarations']
            flags += ['-O2'] if mode == 'normal' else ['-O1', '-g', '-fsanitize=address,undefined', '-fno-sanitize-recover=all', '-fno-omit-frame-pointer']
            for directory in ('lib', 'lib/common', 'tests/fuzz'):
                flags += ['-I', str(upstream / directory)]
            objects = []
            for index, source in enumerate((GENERATOR, upstream / 'tests/fuzz/fuzz_data_producer.c', upstream / 'tests/fuzz/fuzz_helpers.c')):
                obj, dep = output / f'{mode}-{index}.o', output / f'{mode}-{index}.d'
                command(f'{mode}-compile-{index}', [cc, *flags, '-MMD', '-MF', str(dep), '-c', str(source), '-o', str(obj)])
                objects.append(str(obj))
                body = dep.read_text().replace('\\\n', '').split(':', 1)[1]
                for name in shlex.split(body):
                    path = Path(name).resolve()
                    if path == GENERATOR.resolve():
                        continue
                    require(path.is_relative_to(upstream), 'unexpected non-system dependency')
                    relative = path.relative_to(upstream).as_posix()
                    require(pinned.get(relative) == digest(path.read_bytes()), 'included header/source binding')
                    dependencies.add(relative)
            exe = output / mode
            command(mode + '-link', [cc, *flags, *objects, str(library), '-o', str(exe)])
            overrides = {'ASAN_OPTIONS': 'halt_on_error=1:detect_leaks=0', 'UBSAN_OPTIONS': 'halt_on_error=1:print_stacktrace=1'} if mode == 'sanitized' else None
            raw = command(mode + '-capture', [str(exe)], input_data, overrides)
            data, host = unpack(raw)
            (output / (mode + '.tsv')).write_bytes(data)
            observed.append((data, host))
            require({str(path): digest(path.read_bytes()) for path in paths} == before, 'source changed during capture')
        require(all(exact(host, observed[0][1]) and data == observed[0][0] for data, host in observed), 'normal/sanitized equality')
        data, host = observed[0]
        require(digest(data) == OBSERVATIONS_SHA256, 'original C capture identity')
        if expected is not None:
            require(data == expected, 'frozen observations differ')
        capture = {'compiler': report['compiler'], 'host': host,
                   'archive_sha256': digest(library.read_bytes()),
                   'sanitized_equal': args.sanitize,
                   'included_sources': sorted(dependencies),
                   'policy': 'Historical capture host/compiler/archive; regeneration checks exact logical rows and independently records current environment. No native unexecuted-platform claim.'}
        manifest = {'schema_version': 1, 'upstream_revision': REVISION,
                    'generator': GENERATOR.relative_to(ROOT).as_posix(), 'generator_sha256': digest(GENERATOR.read_bytes()),
                    'importer': Path(__file__).resolve().relative_to(ROOT).as_posix(), 'importer_sha256': digest(Path(__file__).read_bytes()),
                    'source_sha256': {name: pinned[name] for name in sorted(dependencies)},
                    'fixtures': [{'path': 'observations.tsv', 'size': len(data), 'sha256': digest(data), 'rows': COUNT}],
                    'expected_rust_classifications': validate(data), 'policy': POLICY, 'capture': capture}
        (output / 'capture-manifest.json').write_text(serialized(manifest))
        if args.capture:
            with (COLLECTION / 'observations.tsv').open('xb') as f:
                f.write(data)
            with (COLLECTION / 'manifest.json').open('x') as f:
                f.write(serialized(manifest))
            offline()
        report.update(status='passed', classifications=validate(data), host=host,
                      observations_sha256=digest(data), source_unchanged=True,
                      fixture_unchanged=expected is None or data == expected)
        print('Verified 81 C selector records; expected Rust admission 57 / unsupported 20 / advanced 4. No C compression was run.')
    except BaseException as error:
        report.update(status='failed', error=repr(error))
        raise
    finally:
        save_report()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--upstream', type=Path)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--cc')
    parser.add_argument('--sanitize', action='store_true')
    parser.add_argument('--capture', action='store_true')
    args = parser.parse_args()
    live = args.upstream is not None or args.output is not None or args.cc is not None or args.sanitize or args.capture
    if args.check and live:
        parser.error('--check is offline and cannot be combined with regeneration arguments')
    if live:
        if args.upstream is None or args.output is None:
            parser.error('regeneration needs --upstream and a fresh external --output')
        regenerate(args)
    else:
        data, _ = offline()
        print('Verified offline selector capture:', validate(data))


if __name__ == '__main__':
    main()
