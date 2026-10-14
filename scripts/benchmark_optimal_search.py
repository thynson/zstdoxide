#!/usr/bin/env python3
"""Build, verify and measure private sorted search; measurements require CPU gates.

All timings are diagnostic, fixed-order desktop observations. No stable speedup
claim follows from them. The executable checks complete returned candidates and
exact requested owner allocations on every round; C map conformance is a separate
captured fixture gate. No network or upstream C execution is performed here.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import statistics
import subprocess
import time
import tomllib

ROOT = Path(__file__).resolve().parents[1]
MODES = {'safe': [], 'tree': ['unsafe-reference-tree'],
         'short': ['unsafe-optimal-short-key'], 'matches': ['unsafe-optimal-matches'],
         'all': None}
MODULES = ('error', 'greedy_key', 'tree_layout', 'reference_tree',
           'optimal_search_layout', 'optimal_short_key', 'optimal_matches', 'optimal_search')


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def save(path, data):
    path.write_text(json.dumps(data, sort_keys=True, indent=2) + '\n')


def records(data, measured):
    rows = [json.loads(line) for line in data.splitlines()]
    need(len(rows) == 5, 'five minimum-match workloads')
    for minimum, row in zip(range(3, 8), rows):
        need(set(row) == {'minimum', 'checksum', 'candidates', 'allocations', 'owner_bytes', 'rounds', 'samples_ns'}, 'exact workload keys')
        need(type(row['minimum']) is int and row['minimum'] == minimum, 'ordered workloads')
        need(type(row['checksum']) is str and len(row['checksum']) == 16 and all(c in '0123456789abcdef' for c in row['checksum']), 'checksum')
        for name in ('candidates', 'allocations', 'owner_bytes', 'rounds'):
            need(type(row[name]) is int and row[name] >= 0, 'integer workload field')
        need(row['candidates'] > 0 and row['owner_bytes'] > 0, 'nonempty exercised workload')
        need(row['allocations'] == (4 if minimum == 3 else 3), 'exact independent owners')
        need(row['rounds'] == (64 if measured else 0), 'fixed identical repetitions')
        samples = row['samples_ns']
        need(type(samples) is list and len(samples) == (5 if measured else 0), 'sample count')
        need(all(type(v) is int and v > 0 for v in samples), 'positive integer timings')
    return rows


def invariant(rows):
    return [{k: v for k, v in row.items() if k not in ('rounds', 'samples_ns')} for row in rows]


def run(out, label, argv, env, cwd):
    log = out / (label + '.log')
    need(not log.exists(), 'refuse to overwrite command evidence')
    start = time.monotonic()
    with log.open('xb') as stream:
        result = subprocess.run(argv, cwd=cwd, env=env, stdout=stream,
                                stderr=subprocess.STDOUT, timeout=600)
    entry = dict(label=label, argv=argv, cwd=str(cwd), exit_code=result.returncode,
                 seconds=time.monotonic() - start, log=log.name, sha256=sha(log))
    need(result.returncode == 0, 'command failed: ' + label)
    return entry


def prepare(out):
    need(not out.exists(), 'prepare requires a fresh output directory')
    out.mkdir(parents=True)
    features = sorted(set(tomllib.loads((ROOT / 'Cargo.toml').read_text())['features']) - {'default', 'std'})
    paths = [f'src/{m}.rs' for m in MODULES] + ['Cargo.toml', 'rust-toolchain.toml',
             'scripts/benchmark_optimal_search.py', 'scripts/support/optimal_search_benchmark.rs',
             'test-support/allocator/Cargo.toml', 'test-support/allocator/src/lib.rs']
    source = {p: sha(ROOT / p) for p in paths}
    for p in paths:
        dest = out / 'snapshot' / p
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / p, dest)
    caller = out / 'caller'
    (caller / 'src').mkdir(parents=True)
    (caller / 'Cargo.toml').write_text('[package]\nname="optimal-search-diagnostic"\nversion="0.0.0"\nedition="2024"\n[features]\nstd=[]\n' + ''.join(f'{f}=[]\n' for f in features) + '\n[dependencies]\nzstdoxide-test-allocator={path=' + json.dumps(str(ROOT / 'test-support/allocator')) + '}\n[profile.release]\noverflow-checks=true\n')
    (caller / 'src/main.rs').write_text((ROOT / 'scripts/support/optimal_search_benchmark.rs').read_text().replace('@ROOT@', str(ROOT)))
    env = dict(os.environ, CARGO_TARGET_DIR=str(out / 'target'), CARGO_INCREMENTAL='0',
               RUSTUP_TOOLCHAIN='nightly-2026-10-08')
    report = dict(status='running', source=source, features=features, commands=[], binaries={}, expected=None)
    save(out / 'prepare.json', report)
    try:
        report['commands'].append(run(out, 'compiler', ['rustc', '-Vv'], env, ROOT))
        for mode, selected in MODES.items():
            selected = features if selected is None else selected
            flags = ['--features', ','.join(selected)] if selected else []
            report['commands'].append(run(out, 'build-' + mode, ['cargo', 'build', '--offline', '--release', *flags], env, caller))
            binary = out / ('search-' + mode)
            shutil.copy2(out / 'target/release/optimal-search-diagnostic', binary)
            report['binaries'][mode] = sha(binary)
            report['commands'].append(run(out, 'verify-' + mode, [str(binary), '--verify'], env, caller))
            observed = invariant(records((out / ('verify-' + mode + '.log')).read_text(), False))
            if report['expected'] is None:
                report['expected'] = observed
            need(observed == report['expected'], 'candidate/owner mismatch')
            save(out / 'prepare.json', report)
        need(source == {p: sha(ROOT / p) for p in paths}, 'source changed during prepare')
        report['status'] = 'passed'
    except BaseException as error:
        report['status'] = 'failed'
        report['error'] = repr(error)
        raise
    finally:
        save(out / 'prepare.json', report)


def measure(out, gates, repeat):
    prepared = json.loads((out / 'prepare.json').read_text())
    need(prepared['status'] == 'passed', 'prepared equivalent binaries required')
    need(prepared['source'] == {p: sha(ROOT / p) for p in prepared['source']}, 'source changed')
    need(len(gates) == 3, 'matrix, foreign Miri and no-atomics completion gates required')
    checked = []
    for path in gates:
        data = json.loads(path.read_text())
        need(data['status'] == 'passed', 'CPU gate incomplete')
        need(all(data['source'].get(f'src/{m}.rs') == prepared['source'][f'src/{m}.rs'] for m in MODULES), 'gate source mismatch')
        checked.append(dict(path=str(path), sha256=sha(path)))
    phase = 'repeat' if repeat else 'first'
    need(not (out / (phase + '.json')).exists(), 'fresh measurement campaign required')
    if repeat:
        need(json.loads((out / 'first.json').read_text())['status'] == 'passed', 'complete first campaign before independent repeat')
    report = dict(status='running', gates=checked, commands=[], results={},
                  qualification='Fixed order, uncontrolled desktop thermal/clocks/background load; diagnostic only, no stable speedup claim.')
    try:
        for mode in (['safe', 'all'] if repeat else MODES):
            binary = out / ('search-' + mode)
            need(sha(binary) == prepared['binaries'][mode], 'executable changed')
            label = phase + '-' + mode
            report['commands'].append(run(out, label, [str(binary)], dict(os.environ), out / 'caller'))
            rows = records((out / (label + '.log')).read_text(), True)
            need(invariant(rows) == prepared['expected'], 'timed output/owner mismatch')
            report['results'][mode] = rows
            save(out / (phase + '.json'), report)
        report['status'] = 'passed'
    except BaseException as error:
        report['status'] = 'failed'
        report['error'] = repr(error)
        raise
    finally:
        save(out / (phase + '.json'), report)


def audit(out):
    prepared = json.loads((out / 'prepare.json').read_text())
    need(prepared['status'] == 'passed', 'prepare status')
    need(all(sha(out / 'snapshot' / p) == h for p, h in prepared['source'].items()), 'frozen source hashes')
    expected = None
    for mode in MODES:
        need(sha(out / ('search-' + mode)) == prepared['binaries'][mode], 'binary hash')
        observed = invariant(records((out / ('verify-' + mode + '.log')).read_text(), False))
        if expected is None:
            expected = observed
        need(observed == expected == prepared['expected'], 'independent baseline agreement')
    reports = [prepared]
    ratios = {}
    for phase, modes in [('first', list(MODES)), ('repeat', ['safe', 'all'])]:
        data = json.loads((out / (phase + '.json')).read_text())
        need(data['status'] == 'passed' and set(data['results']) == set(modes), 'complete campaign')
        reports.append(data)
        for mode in modes:
            rows = records((out / (phase + '-' + mode + '.log')).read_text(), True)
            need(rows == data['results'][mode] and invariant(rows) == expected, 'exact measured records')
        ratios[phase] = [statistics.median(a['samples_ns']) / statistics.median(s['samples_ns'])
                         for s, a in zip(data['results']['safe'], data['results']['all'])]
    for report in reports:
        for command in report['commands']:
            name = command['log']
            need(Path(name).name == name and command['exit_code'] == 0 and sha(out / name) == command['sha256'], 'complete command logs')
    return dict(status='passed', workloads=5, verified_profiles=5, measured_profile_runs=7,
                samples=175, ratios_all_over_safe=ratios,
                qualification='Candidate checksum and exact allocator equivalence; full C maps qualified separately. Diagnostic timings only.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument('--prepare', action='store_true')
    action.add_argument('--measure', action='store_true')
    action.add_argument('--repeat', action='store_true')
    action.add_argument('--audit', action='store_true')
    parser.add_argument('--gate', type=Path, action='append', default=[])
    args = parser.parse_args()
    out = args.output.resolve()
    if args.prepare:
        prepare(out)
    elif args.audit:
        print(json.dumps(audit(out), sort_keys=True))
    else:
        measure(out, args.gate, args.repeat)


if __name__ == '__main__':
    main()
