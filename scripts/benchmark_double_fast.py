#!/usr/bin/env python3
"""Sixteen fixed Fast/DoubleFast cases, clock-free by default; --timed explicitly enables clocks.

All requested executables are built before any measurement. Allocation tracking,
complete-frame checks and decompression run outside clocks. --validate-only is
purely offline and validates recorded evidence without rewriting it.
"""
from pathlib import Path
import argparse
import csv
import datetime
import hashlib
import itertools
import json
import platform
import statistics
import subprocess
import sys
import tomllib

sys.dont_write_bytecode = True
# Reuse the existing environment whitelist, Cargo input inventory and strict JSON
# parser, without executing the separate named-search campaign.
import benchmark_search as shared

ROOT = Path(__file__).resolve().parents[1]
TOOLCHAIN = 'nightly-2026-10-08'
FEATURES = shared.FEATURES
PROFILES = ('level-1', 'level-3', 'long-18-short-6', 'long-6-short-18')
WORKLOADS = ('far-history', 'skewed', 'history', 'mixed')
require, exact, sha = shared.require, shared.exact, shared.sha
SEED = 0x5eedfa5720261009
INPUT_PINS = {'far-history': {'bytes': 2228224, 'fnv1a64': '2023247e1659a142', 'sha256': '71ba4eaf6ed525768048f439ba3341e20f8a17c52b1503e540aa508d5a98a78c'}, 'skewed': {'bytes': 32768, 'fnv1a64': 'eb099b718ab7e75b', 'sha256': '613ad2caf8e6b161d48f0ce4aee8f29eaa1c45b9978876a4f9577e149ef607b2'}, 'history': {'bytes': 393216, 'fnv1a64': '63af07be4a3a8c7c', 'sha256': '9cc84f81533a4a4d2e16b59f04213ff5da07e716391b0d8d6314d32345937c98'}, 'mixed': {'bytes': 524288, 'fnv1a64': 'b3aeb6510bbe33c8', 'sha256': 'b2153d35b896544d8086e4bb03600fdc07d3e84723a26545a878a45c7a275ce3'}}


def corpus():
    mask = (1 << 64) - 1
    def next_byte(state):
        state ^= (state << 13) & mask
        state ^= state >> 7
        state ^= (state << 17) & mask
        return state, state & 255
    def noise(state, count):
        result = bytearray()
        for _ in range(count):
            state, byte = next_byte(state)
            result.append(byte)
        return result
    def skewed(count):
        state, result = SEED, bytearray()
        phrase = b'Reference Fast one-shot benchmark with skewed literal frequencies.\n'
        for i in range(count):
            state, byte = next_byte(state)
            result.append(result[i - 8192] if i >= 8192 and i % 509 != 0 else
                          phrase[i % len(phrase)] if i % 11 < 8 else byte)
        return result
    a, b, text = noise(SEED ^ 1, 131072), noise(SEED ^ 2, 131072), skewed(131072)
    return {name: {'bytes': len(data), 'fnv1a64': shared.fnv(data),
                   'sha256': hashlib.sha256(data).hexdigest()}
            for name, data in [('far-history', a + noise(SEED ^ 3, 11 * 131072) + a + text * 4), ('skewed', skewed(32768)), ('history', a + b + a),
                               ('mixed', text + b + b'z' * 131072 + text)]}


def source_manifest():
    paths = set((ROOT / 'src').rglob('*.rs')) | set((ROOT / 'test-support/allocator/src').rglob('*.rs'))
    paths.update(ROOT / name for name in ('Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
        'test-support/allocator/Cargo.toml', 'examples/double_fast_benchmark.rs',
        'scripts/benchmark_double_fast.py', 'scripts/benchmark_search.py',
        'scripts/check_reference_fast_baseline.py', 'tests/fixtures/reference-fast-baseline/expected.tsv',
        'tests/fixtures/reference-fast-baseline/frames.bin', 'tests/fixtures/reference-fast-baseline/provenance.json'))
    return {p.relative_to(ROOT).as_posix(): sha(p) for p in sorted(paths)}


def write(path, value):
    path.write_text(json.dumps(value, sort_keys=True, indent=2, allow_nan=False) + '\n')


def resolved(workload, profile):
    # Pinned v1.5.7 clevels.h levels1/3, >256KiB and <=128KiB rows.
    # All overrides are followed by known-size normalization without a dictionary.
    small = workload == 'skewed'
    if profile == 'level-1':
        return [15, 12, 13, 1, 6, 0, 1] if small else [19, 13, 14, 1, 7, 0, 1]
    window = 15 if small else 21 if workload == 'far-history' else 19
    if profile == 'level-3':
        return [window, 15, 16, 2, 5, 0, 2] if small else [window, 16, 17, 1, 5, 0, 2]
    chain, table = (6, 18) if profile == 'long-18-short-6' else (18, 6)
    return [window, min(chain, window), min(table, window + 1), 2 if small else 1, 4, 0, 2]


def rows(path, config, timed):
    data = [shared.parse_json(line) for line in path.read_text().splitlines()]
    expected_features = [] if config == 'safe' else list(FEATURES) if config == 'all' else [config]
    require(len(data) == 17 and exact(data[0], dict(record='metadata', schema_version=1,
        features=expected_features, seed=f'{SEED:016x}', timed=timed, samples=5,
        minimum_ms=100, expected_rows=16)), 'metadata / exact rows')
    expected = list(itertools.product(WORKLOADS, PROFILES))
    for row, (workload, profile) in zip(data[1:], expected, strict=True):
        require(set(row) == {'record', 'workload', 'profile', 'source_bytes', 'source_fnv1a64',
            'resolved', 'frame_bytes', 'frame_fnv1a64', 'allocation', 'samples'}, 'row schema')
        require(row['record'] == 'case' and (row['workload'], row['profile']) == (workload, profile), 'case order')
        pin = INPUT_PINS[workload]
        require(type(row['source_bytes']) is int and row['source_bytes'] == pin['bytes']
                and row['source_fnv1a64'] == pin['fnv1a64'], 'independent complete source identity')
        require(exact(row['resolved'], resolved(workload, profile)), 'independent resolved parameters')
        require(type(row['frame_bytes']) is int and 0 < row['frame_bytes'] <= pin['bytes'] + 128, 'raw frame bound')
        require(type(row['frame_fnv1a64']) is str and len(row['frame_fnv1a64']) == 16
                and all(c in '0123456789abcdef' for c in row['frame_fnv1a64']), 'frame fingerprint')
        a = row['allocation']
        fields = {'attempts', 'allocate_calls', 'grow_calls', 'peak_live_blocks', 'peak_requested_bytes',
                  'peak_bytes', 'returned_requested_bytes', 'returned_bytes', 'identities_created',
                  'owners_dropped', 'final_live_blocks', 'final_live_bytes', 'violations'}
        require(type(a) is dict and set(a) == fields and a['violations'] is False, 'allocator schema / violations')
        require(all(type(a[k]) is int and a[k] >= 0 for k in fields - {'violations'}), 'allocator counter types')
        require(a['attempts'] > 0 and a['allocate_calls'] > 0 and a['peak_live_blocks'] > 1
                and a['attempts'] == a['allocate_calls'] + a['grow_calls']
                and a['final_live_blocks'] == a['final_live_bytes'] == 0
                and a['identities_created'] == a['owners_dropped'] > 1
                and row['frame_bytes'] <= a['returned_requested_bytes'] <= a['returned_bytes'] <= a['peak_bytes']
                and a['returned_requested_bytes'] <= a['peak_requested_bytes'] <= a['peak_bytes'], 'allocator consistency')
        require(type(row['samples']) is list and len(row['samples']) == (5 if timed else 0), 'sample count')
        for sample in row['samples']:
            require(type(sample) is dict and set(sample) == {'iterations', 'elapsed_ns'}
                    and type(sample['iterations']) is int and sample['iterations'] > 0
                    and type(sample['elapsed_ns']) is int and sample['elapsed_ns'] >= 100000000, 'sample bounds')
    return data[1:]


def summarize(results, timed):
    values = {}
    for config, cases in results.items():
        output = []
        for row in cases:
            costs = [s['elapsed_ns'] / s['iterations'] / row['source_bytes'] for s in row['samples']]
            output.append(dict(workload=row['workload'], profile=row['profile'], frame_bytes=row['frame_bytes'],
                compressed_size_ratio=row['frame_bytes'] / row['source_bytes'],
                allocation=row['allocation'], timing=None if not timed else {
                    'minimum_ns_per_byte': min(costs), 'median_ns_per_byte': statistics.median(costs),
                    'maximum_ns_per_byte': max(costs),
                    'median_mib_per_second': statistics.median(1e9 / c / 1048576 for c in costs)}))
        values[config] = output
    return values


def build_command(config, host):
    argv = ['cargo', '+' + TOOLCHAIN, 'build', '--offline', '--locked', '--release',
            '--example', 'double_fast_benchmark', '--target', host]
    if config == 'all':
        argv.append('--all-features')
    elif config != 'safe':
        argv += ['--features', config]
    return argv


def check_fast_baseline(output, evidence, results):
    # Verify the immutable pre-DoubleFast source independently before comparing.
    import check_reference_fast_baseline as baseline
    baseline.audit(ROOT)
    collection = ROOT / baseline.COLLECTION
    packed = (collection / 'frames.bin').read_bytes()
    provenance = shared.load_json(collection / 'provenance.json')
    records = list(csv.DictReader((collection / 'expected.tsv').read_text().splitlines(), delimiter='\t'))
    for row in records:
        if row['profile'] != 'level-1':
            continue
        offset, length = int(row['frame_offset']), int(row['frame_bytes'])
        name = row['workload'] + '-level-1.zst'
        require((output / 'frames/safe' / name).read_bytes() == packed[offset:offset + length],
                'immutable complete Fast baseline')
        if evidence['host'] == provenance['target'] and evidence['compiler'] == provenance['compiler']:
            observed = next(r for r in results['safe'] if (r['workload'], r['profile']) == (row['workload'], 'level-1'))
            require(all(observed['allocation'][field] == int(row[field]) for field in baseline.FIELDS),
                    'qualified immutable Fast allocation baseline')


def validate(output, evidence):
    require(evidence['status'] == 'passed' and exact(evidence['source'], source_manifest())
            and exact(evidence['inputs'], INPUT_PINS), 'complete unchanged campaign inputs')
    configs, timed = evidence['configs'], evidence['timed']
    require(type(timed) is bool and configs[0] == 'safe' and len(configs) == len(set(configs))
            and set(configs) <= {'safe', 'all', *FEATURES}, 'configuration set')
    require(evidence['toolchain'] == TOOLCHAIN and evidence['scope'] == 'Global one-shot compression and output destruction; allocation survey and validation outside clocks', 'measurement scope')
    require(evidence['source_unchanged'] is True, 'completed source check')
    base, host = evidence['base_environment'], evidence['host']
    require(type(base) is dict and set(base) <= shared.BASE_ENV_KEYS | set(shared.FORCED_ENV)
            and all(type(k) is str and type(v) is str for k, v in base.items())
            and all(base.get(k) == v for k, v in shared.FORCED_ENV.items()), 'explicit effective environment')
    require(exact(evidence['cargo_configs'], shared.cargo_config_manifest(ROOT, base)), 'Cargo configuration input provenance')
    require(type(host) is str and host and '/' not in host and '\\' not in host
            and evidence['compiler'] == (output / 'compiler.stdout').read_text()
            and ('host: ' + host) in evidence['compiler'].splitlines()
            and (output / 'host.stdout').read_text().strip() == host, 'compiler host binding')
    require(set(evidence['executables']) == set(configs), 'executable inventory')
    require(set(evidence['frames']) == set(configs), 'complete frame inventory')
    campaign = Path(evidence['campaign_directory'])
    require(campaign.is_absolute(), 'original campaign directory')
    results = {}
    commands = evidence['commands']
    require(len(commands) == 5 + 2 * len(configs), 'complete command evidence')
    labels = ['compiler', 'compiler-path', 'head', 'status', 'host'] + ['build-' + c for c in configs] + ['run-' + c for c in configs]
    setup_commands = [
        ['rustc', '+' + TOOLCHAIN, '-vV'],
        ['rustup', 'which', '--toolchain', TOOLCHAIN, 'rustc'],
        ['git', 'rev-parse', 'HEAD'], ['git', 'status', '--short'],
        ['rustc', '+' + TOOLCHAIN, '--print', 'host-tuple'],
    ]
    for command, label in zip(commands, labels, strict=True):
        require(command['label'] == label and type(command['exit_code']) is int
                and command['exit_code'] == 0 and command['exception'] is None,
                'successful ordered commands')
        require(command['cwd'] == str(ROOT), 'command working directory')
        for stream in ('stdout', 'stderr'):
            path = output / (label + '.' + stream)
            require(command[stream] == path.name and command[stream + '_sha256'] == sha(path), 'command log binding')
        expected_env = base
        if label.startswith('build-'):
            config = label.removeprefix('build-')
            argv = build_command(config, host)
            expected_env = base | {'CARGO_TARGET_DIR': str(campaign / 'targets' / config), 'CARGO_TERM_COLOR': 'never'}
        elif label.startswith('run-'):
            config = label.removeprefix('run-')
            executable = campaign / 'targets' / config / host / 'release/examples/double_fast_benchmark'
            recorded = evidence['executables'][config]
            require(set(recorded) == {'path', 'sha256'} and recorded['path'] == str(executable)
                    and type(recorded['sha256']) is str and len(recorded['sha256']) == 64
                    and all(c in '0123456789abcdef' for c in recorded['sha256']), 'canonical executable identity')
            if executable.exists():
                require(sha(executable) == recorded['sha256'], 'existing executable hash')
            argv = [str(executable)]
            expected_env = base | {'ZSTDOXIDE_DOUBLE_FAST_BENCH_TIMED': '1' if timed else '0',
                'ZSTDOXIDE_DOUBLE_FAST_BENCH_FRAMES': str(campaign / 'frames' / config)}
        else:
            argv = setup_commands[labels.index(label)]
        require(exact(command['argv'], argv) and exact(command['environment'], expected_env), 'recorded command/effective environment')
    compiler_path = Path((output / 'compiler-path.stdout').read_text().strip())
    require(compiler_path.is_absolute() and len(evidence['compiler_sha256']) == 64, 'compiler executable metadata')
    if compiler_path.is_file():
        require(sha(compiler_path) == evidence['compiler_sha256'], 'existing compiler executable hash')
    for config in configs:
        result = rows(output / ('run-' + config + '.stdout'), config, timed)
        names = {r['workload'] + '-' + r['profile'] + '.zst' for r in result}
        directory = output / 'frames' / config
        require(set(evidence['frames'][config]) == names
                and {p.name for p in directory.iterdir()} == names, 'exact complete frame set')
        for row in result:
            name = row['workload'] + '-' + row['profile'] + '.zst'
            data = (directory / name).read_bytes()
            require(exact(evidence['frames'][config][name], {'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()})
                    and len(data) == row['frame_bytes'] and shared.fnv(data) == row['frame_fnv1a64'], 'complete saved frame binding')
            if config != 'safe':
                require(data == (output / 'frames/safe' / name).read_bytes(), 'complete backend frame bytes differ')
        if results:
            require(exact([{k:v for k,v in r.items() if k != 'samples'} for r in result],
                          [{k:v for k,v in r.items() if k != 'samples'} for r in results['safe']]), 'backend frames/allocations differ')
        results[config] = result
    require(exact(evidence['summary'], summarize(results, timed)), 'summary arithmetic')
    check_fast_baseline(output, evidence, results)
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--configs', default=','.join(('safe', *FEATURES, 'all')))
    parser.add_argument('--timed', action='store_true')
    parser.add_argument('--validate-only', action='store_true')
    args = parser.parse_args()
    require(exact(corpus(), INPUT_PINS), 'independently pinned corpus')
    output = args.output.resolve()
    if args.validate_only:
        evidence = shared.load_json(output / 'evidence.json')
        validate(output, evidence)
        print('Verified complete DoubleFast benchmark evidence offline.')
        return
    require(not output.exists() and not output.is_relative_to(ROOT) and not ROOT.is_relative_to(output), 'fresh external output required')
    configs = args.configs.split(',')
    require(configs[0] == 'safe' and len(configs) == len(set(configs)) and set(configs) <= {'safe', 'all', *FEATURES}, 'unique configurations starting with safe')
    require(set(tomllib.loads((ROOT / 'Cargo.toml').read_text())['features']) - {'std', 'default'} == set(FEATURES), 'complete feature inventory')
    source, base = source_manifest(), shared.effective_environment()
    cargo_configs = shared.cargo_config_manifest(ROOT, base)
    output.mkdir(parents=True)
    evidence = dict(status='running', toolchain=TOOLCHAIN, source=source, inputs=INPUT_PINS,
        configs=configs, timed=args.timed, platform=platform.platform(), base_environment=base,
        campaign_directory=str(output),
        cargo_configs=cargo_configs, commands=[], executables={}, frames={},
        scope='Global one-shot compression and output destruction; allocation survey and validation outside clocks',
        started_utc=datetime.datetime.now(datetime.timezone.utc).isoformat())
    def save():
        write(output / 'evidence.json', evidence)
    def unchanged():
        require(exact(source_manifest(), source) and exact(shared.cargo_config_manifest(ROOT, base), cargo_configs), 'source/Cargo config changed')
    def command(label, argv, env):
        unchanged()
        stdout, stderr = output / (label + '.stdout'), output / (label + '.stderr')
        status, exception = None, None
        with stdout.open('xb') as out, stderr.open('xb') as err:
            try:
                process = subprocess.run(argv, cwd=ROOT, env=env, stdout=out, stderr=err, timeout=300, check=False)
                status = process.returncode
            except (OSError, subprocess.TimeoutExpired) as error:
                exception = repr(error)
        evidence['commands'].append(dict(label=label, argv=list(map(str, argv)), cwd=str(ROOT), environment=env,
            exit_code=status, exception=exception, stdout=stdout.name, stdout_sha256=sha(stdout), stderr=stderr.name, stderr_sha256=sha(stderr)))
        save()
        require(status == 0 and exception is None, 'command failed: ' + label)
        unchanged()
        return stdout.read_text()
    try:
        evidence['compiler'] = command('compiler', ['rustc', '+' + TOOLCHAIN, '-vV'], base)
        compiler = command('compiler-path', ['rustup', 'which', '--toolchain', TOOLCHAIN, 'rustc'], base).strip()
        evidence['compiler_sha256'] = sha(compiler)
        command('head', ['git', 'rev-parse', 'HEAD'], base)
        command('status', ['git', 'status', '--short'], base)
        host = command('host', ['rustc', '+' + TOOLCHAIN, '--print', 'host-tuple'], base).strip()
        evidence['host'] = host
        target = output / 'targets'
        for config in configs:
            env = base | {'CARGO_TARGET_DIR': str(target / config), 'CARGO_TERM_COLOR': 'never'}
            args_build = build_command(config, host)
            command('build-' + config, args_build, env)
            executable = target / config / host / 'release/examples/double_fast_benchmark'
            evidence['executables'][config] = dict(path=str(executable), sha256=sha(executable))
            print('Built DoubleFast benchmark:', config, flush=True)
        results = {}
        for config in configs:
            exe = evidence['executables'][config]
            require(sha(exe['path']) == exe['sha256'], 'executable changed')
            directory = output / 'frames' / config
            directory.mkdir(parents=True)
            env = base | {'ZSTDOXIDE_DOUBLE_FAST_BENCH_TIMED': '1' if args.timed else '0',
                'ZSTDOXIDE_DOUBLE_FAST_BENCH_FRAMES': str(directory)}
            command('run-' + config, [exe['path']], env)
            require(sha(exe['path']) == exe['sha256'], 'executable changed during run')
            results[config] = rows(output / ('run-' + config + '.stdout'), config, args.timed)
            evidence['frames'][config] = {p.name: {'bytes': p.stat().st_size, 'sha256': sha(p)}
                                          for p in sorted(directory.iterdir())}
            print('Measured DoubleFast benchmark:' if args.timed else 'Verified clock-free DoubleFast smoke:', config, flush=True)
        evidence.update(status='passed', summary=summarize(results, args.timed), source_unchanged=True)
        save()
        validate(output, evidence)
    except BaseException as error:
        evidence.update(status='failed', error=repr(error))
        save()
        raise
    print('Verified sixteen complete cases across', len(configs), 'configurations.')


if __name__ == '__main__':
    main()
