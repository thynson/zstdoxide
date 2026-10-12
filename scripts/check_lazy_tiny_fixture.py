#!/usr/bin/env python3
"""Replay immutable direct Lazy/Lazy2 observations without executing a codec."""
from pathlib import Path
import argparse
import hashlib
import json

ROOT = Path(__file__).resolve().parents[1]
COLLECTION = 'tests/fixtures/reference-lazy-tiny'
FIXTURE_SHA = '5f7cf83d440e22cf581b59b7e1b592b9b2db4b2d522ac9efcc57b7753f5736dc'
CHECKER_SHA = '950e9c2ab5e864d8c1fce5d8e9f5e0b67a0c3b7ffe20d77b2a880920637a2069'
AUDITOR_SHA = 'ba5462b9fc26f8471b5b465009c3068b673a3985172d68812ae594ce2fd2e994'


def require(value, reason):
    if not value:
        raise ValueError(reason)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def bounded(name, cap):
    p = ROOT / name
    require(p.is_file() and not p.is_symlink() and p.stat().st_size <= cap,
            'bounded regular fixture: ' + name)
    data = p.read_bytes()
    require(len(data) <= cap, 'fixture grew beyond cap')
    return data


def fixture():
    for name, digest in {'scripts/check_lazy_tiny_recipes.py': CHECKER_SHA,
                         'scripts/check_lazy_tiny_capture.py': AUDITOR_SHA}.items():
        require(sha(bounded(name, 1 << 20)) == digest, 'reviewed checker identity')
    import check_lazy_tiny_recipes as checker
    import check_lazy_tiny_capture as auditor
    g, cases, _ = checker.inputs()
    raw = bounded(COLLECTION + '/capture.json', 1 << 20)
    require(sha(raw) == FIXTURE_SHA, 'immutable capture identity')
    accepted = g.decode_json(raw)
    require(g.exact(accepted['schema_version'], 1) and accepted['upstream_revision'] == g.REVISION
            and accepted['auditor_sha256'] == AUDITOR_SHA, 'capture schema and provenance')
    local = set(checker.PINS) | {checker.MANIFEST, 'scripts/check_lazy_tiny_recipes.py',
                               'scripts/capture_lazy_tiny.py', 'tests/upstream/manifest.json'}
    require(g.exact(accepted['source'], {n: sha(bounded(n, 2 << 20)) for n in sorted(local)}),
            'unchanged captured source closure')
    pack = bounded(COLLECTION + '/observations.ndjson', 1 << 20)
    require(sha(pack) == accepted['pack_sha256'] and g.exact(len(pack), accepted['pack_bytes']),
            'complete observation pack')
    names = [c.name for c in cases] + [f'salt-{mode}-{d}' for mode in ('normal', 'sanitized') for d in range(3)]
    require(set(accepted['records']) == set(names), 'complete observation inventory')
    cursor, observed = 0, {}
    for name in names:
        r = accepted['records'][name]
        require(set(r) == {'offset', 'bytes', 'sha256'} and g.exact(r['offset'], cursor),
                'consecutive observation extent')
        size = g.integer(r['bytes'], 1, len(pack) - cursor, 'observation size')
        data = pack[cursor:cursor + size]
        require(sha(data) == r['sha256'], 'observation identity')
        observed[name] = data
        cursor += size
    require(cursor == len(pack), 'no unused observation bytes')
    for c in cases:
        g.validate(c, observed[c.name])
    peaks = {}
    for mode in ('normal', 'sanitized'):
        peaks[mode] = {}
        for d in range(3):
            rows = g.validate_salt(observed[f'salt-{mode}-{d}'], d)
            peaks[mode][str(d)] = [r['peak_guarded_bytes'] for r in rows[1:3]]
            if mode == 'sanitized':
                old = g.records(observed[f'salt-normal-{d}'])
                semantic = lambda records: [{k: v for k, v in r.items() if k != 'peak_guarded_bytes'} for r in records]
                require(g.exact(semantic(rows), semantic(old)), 'cross-build salt semantics')
    raw = {('normal-case-' + c.name, 'stdout'): observed[c.name] for c in cases}
    require(g.exact(auditor.compare_predecessor(g, cases, raw), accepted['audit']['predecessor_controls']),
            'frozen predecessor observations')
    require(g.exact(auditor.observed_geometry(g, cases, raw), accepted['audit']['geometry_observations']),
            'frozen tail observations')
    require(g.exact(peaks, accepted['audit']['context_peak_guarded_bytes']), 'per-mode context peaks')
    return accepted, observed


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true', help='offline replay (the only mode)')
    parser.parse_args()
    try:
        accepted, observed = fixture()
    except (ValueError, OSError, TypeError, KeyError) as error:
        parser.exit(1, 'Lazy fixture verification failed: ' + str(error) + '\n')
    print(json.dumps({'direct_recipes': 138, 'stored_observation_groups': len(observed),
                      'pack_bytes': accepted['pack_bytes'], 'predecessor_controls': 36,
                      'tail_cases': 30, 'child_processes': 0}, sort_keys=True))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
