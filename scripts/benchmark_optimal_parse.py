#!/usr/bin/env python3
"""Build, verify and measure private scripted path decisions; measurements require CPU gates.

All timings are diagnostic, fixed-order desktop observations. No stable speedup
claim follows from them. The executable checks C-derived event/final-state checksums and
exact requested owner allocations on every round; Complete intermediate C state conformance is a separate
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
MODES = {'safe': [], 'frequencies': ['unsafe-optimal-frequencies'],
         'matches': ['unsafe-optimal-matches'], 'paths': ['unsafe-optimal-paths'], 'all': None}
MODULES = ('error', 'constants', 'greedy_key', 'tree_layout', 'optimal_search_layout',
           'optimal_price', 'optimal_frequencies', 'optimal_matches', 'optimal_path_layout', 'optimal_paths', 'optimal_parse')
WORKLOADS = ['path_all_literals', 'path_repeat_publication', 'path_script_seed_1',
             'path_wide_later_4096', 'path_wide_literal_pricing']


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def save(path, data):
    path.write_text(json.dumps(data, sort_keys=True, indent=2) + '\n')


def records(data, measured):
    rows = [json.loads(line) for line in data.splitlines()]
    need(len(rows) == 5, 'five scripted path workloads')
    for name, row in zip(WORKLOADS, rows):
        need(set(row) == {'case', 'checksum', 'events', 'allocations', 'owner_bytes', 'rounds', 'samples_ns'}, 'exact workload keys')
        need(type(row['case']) is str and row['case'] == name, 'ordered workloads')
        need(type(row['checksum']) is str and len(row['checksum']) == 16 and all(c in '0123456789abcdef' for c in row['checksum']), 'checksum')
        for field in ('events', 'allocations', 'owner_bytes', 'rounds'):
            need(type(row[field]) is int and row[field] >= 0, 'integer workload field')
        need(row['events'] == [68,36,99,2076,2087][WORKLOADS.index(name)] and row['owner_bytes'] == 149072, 'exact C events and owner extent')
        need(row['allocations'] == 3, 'exact independent owners')
        need(row['rounds'] == (32 if measured else 0), 'fixed identical repetitions')
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


def source_paths():
    return [f'src/{m}.rs' for m in MODULES] + ['Cargo.toml', 'rust-toolchain.toml',
             'scripts/benchmark_optimal_parse.py', 'scripts/support/optimal_parse_benchmark.rs',
             'test-support/allocator/Cargo.toml', 'test-support/allocator/src/lib.rs',
             'tests/support/optimal_path_fixture.rs', 'tests/fixtures/reference-optimal-paths/cases.bin',
             'tests/fixtures/reference-optimal-paths/provenance.json']


def prepare(out):
    need(not out.exists(), 'prepare requires a fresh output directory')
    out.mkdir(parents=True)
    features = sorted(set(tomllib.loads((ROOT / 'Cargo.toml').read_text())['features']) - {'default', 'std'})
    paths = source_paths()
    source = {p: sha(ROOT / p) for p in paths}
    for p in paths:
        dest = out / 'snapshot' / p
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / p, dest)
    caller = out / 'caller'
    (caller / 'src').mkdir(parents=True)
    (caller / 'Cargo.toml').write_text('[package]\nname="optimal-parse-diagnostic"\nversion="0.0.0"\nedition="2024"\n[features]\nstd=[]\n' + ''.join(f'{f}=[]\n' for f in features) + '\n[dependencies]\nzstdoxide-test-allocator={path=' + json.dumps(str(ROOT / 'test-support/allocator')) + '}\n[profile.release]\noverflow-checks=true\n')
    (caller / 'src/main.rs').write_text((ROOT / 'scripts/support/optimal_parse_benchmark.rs').read_text().replace('@ROOT@', str(ROOT)))
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
            binary = out / ('parse-' + mode)
            shutil.copy2(out / 'target/release/optimal-parse-diagnostic', binary)
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
    (out/'gates').mkdir(exist_ok=True)
    for index,path in enumerate(gates):
        data = json.loads(path.read_text())
        validate_gate(data,index,prepared)
        relative='gates/'+['matrix','miri','thumb'][index]+'.json'
        destination=out/relative
        if destination.exists():need(destination.read_bytes()==path.read_bytes(),'unchanged frozen gate')
        else:shutil.copyfile(path,destination)
        checked.append(dict(path=relative,source=str(path),sha256=sha(destination)))
    phase = 'repeat' if repeat else 'first'
    need(not (out / (phase + '.json')).exists(), 'fresh measurement campaign required')
    if repeat:
        need(json.loads((out / 'first.json').read_text())['status'] == 'passed', 'complete first campaign before independent repeat')
    report = dict(status='running', gates=checked, commands=[], results={},
                  qualification='Fixed order, uncontrolled desktop thermal/clocks/background load; diagnostic only, no stable speedup claim.')
    try:
        for mode in (['safe', 'all'] if repeat else MODES):
            binary = out / ('parse-' + mode)
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


def validate_gate(data,index,prepared):
    need(data['status']=='passed','CPU gate incomplete')
    need(all(data['source'].get(f'src/{m}.rs')==prepared['source'][f'src/{m}.rs'] for m in MODULES),'gate source mismatch')
    if index==0:
        need(data['features']==prepared['features'],'all isolated features covered')
        labels=[label+'-'+feature for feature in data['features'] for label in ('test','nostd')]+['full-safe','full-all']
    elif index==1:
        labels=[target+'-'+mode+'-'+str(i) for target in ('i686-unknown-linux-gnu','s390x-unknown-linux-gnu') for mode in ('safe','paths','all') for i in range(4)]
    else:
        labels=[kind+'-'+mode for kind in ('library','caller') for mode in ('safe','frequencies','paths','matches','all')]
    need([c['label']for c in data['commands']]==labels,'complete ordered gate commands')
    need(all(type(c['exit_code'])is int and c['exit_code']==0 for c in data['commands']),'successful gate commands')


def audit(out):
    prepared = json.loads((out / 'prepare.json').read_text())
    need(prepared['status'] == 'passed', 'prepare status')
    need(set(prepared['source'])==set(source_paths()),'exact bounded source inventory')
    need(set(prepared['binaries'])==set(MODES),'exact built modes')
    features=sorted(set(tomllib.loads((out/'snapshot/Cargo.toml').read_text())['features'])-{'default','std'})
    need(prepared['features']==features,'exact feature inventory')
    need(all(sha(out / 'snapshot' / p) == h for p, h in prepared['source'].items()), 'frozen source hashes')
    expected = None
    for mode in MODES:
        need(sha(out / ('parse-' + mode)) == prepared['binaries'][mode], 'binary hash')
        observed = invariant(records((out / ('verify-' + mode + '.log')).read_text(), False))
        if expected is None:
            expected = observed
        need(observed == expected == prepared['expected'], 'independent baseline agreement')
    reports = [(prepared,['compiler']+[label+'-'+mode for mode in MODES for label in ('build','verify')])]
    frozen_gates=None
    ratios = {}
    for phase, modes in [('first', list(MODES)), ('repeat', ['safe', 'all'])]:
        data = json.loads((out / (phase + '.json')).read_text())
        need(data['status'] == 'passed' and set(data['results']) == set(modes), 'complete campaign')
        reports.append((data,[phase+'-'+mode for mode in modes]))
        need(type(data['gates'])is list and len(data['gates'])==3,'three frozen CPU gates')
        for index,gate in enumerate(data['gates']):
            relative='gates/'+['matrix','miri','thumb'][index]+'.json'
            need(gate['path']==relative and sha(out/relative)==gate['sha256'],'bounded frozen gate identity')
            validate_gate(json.loads((out/relative).read_text()),index,prepared)
        if frozen_gates is None:frozen_gates=data['gates']
        need(data['gates']==frozen_gates,'same completed gates across campaigns')
        for mode in modes:
            rows = records((out / (phase + '-' + mode + '.log')).read_text(), True)
            need(rows == data['results'][mode] and invariant(rows) == expected, 'exact measured records')
        ratios[phase] = [statistics.median(a['samples_ns']) / statistics.median(s['samples_ns'])
                         for s, a in zip(data['results']['safe'], data['results']['all'])]
    for report,labels in reports:
        need([c['label']for c in report['commands']]==labels,'complete ordered command inventory')
        for command in report['commands']:
            name = command['log']
            need(name==command['label']+'.log' and Path(name).name == name and type(command['exit_code'])is int and command['exit_code'] == 0 and sha(out / name) == command['sha256'], 'complete command logs')
    return dict(status='passed', workloads=5, verified_profiles=5, measured_profile_runs=7,
                samples=175, ratios_all_over_safe=ratios,
                qualification='C-derived event/final-state checksum and exact allocator equivalence; all intermediate C maps qualified separately. Diagnostic timings only.')


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
