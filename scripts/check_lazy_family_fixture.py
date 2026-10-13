#!/usr/bin/env python3
"""Replay immutable direct Lazy/Lazy2 observations without executing a codec."""
from pathlib import Path
import argparse
import hashlib
import json

ROOT = Path(__file__).resolve().parents[1]
COLLECTION = 'tests/fixtures/reference-lazy-families'
FIXTURE_SHA = '153efc6849613cb6815d0d0980411da5eb0286ed8f5378cca7479f4e5c01a4de'
CHECKER_SHA = '1269920726bf2a4210cdc05b4777e9eddfaa93a34a5c3bd1c5a47b43ac5ac718'
AUDITOR_SHA = '8021050285b64f04d0495cbabd60d4e4eb2ec701892be61931e59cd590293921'


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


def score_observations(g, cases, recipes, observed):
    """Reconcile placed copies with final C output, without reconstructing probes.

    Threshold classifications apply only to the constructed disjoint copies.
    Complete maps remain in the fixture; these final outputs do not prove an
    ordered trace, nor initial table selection in the baseline repeat family.
    """
    import check_lazy_family_recipes as checker
    checker.check_score_sources(cases, recipes['score_hypotheses'])
    results = []
    by_name = {c.name: c for c in cases}
    for h in recipes['score_hypotheses']:
        c = by_name[h['id']]
        row = g.records(observed[c.name])[1]
        require(len(row['sequences']) == 1, 'one constructed sequence: ' + c.name)
        sequence = row['sequences'][0]
        emitted = [sequence['ll'], sequence['ml'], sequence['off_base']]
        if 'baseline_score' in h:
            candidate = [h['query_delta'], h['candidate_length'], h['candidate_offbase']]
            baseline = [0, h['baseline_length'], h['baseline_offbase']]
            active = c.depth >= h['query_delta']
            early_repeat = c.depth == 0 and h['query_delta'] == 1 and h['candidate_offbase'] == 1
            wins = (active and h['arithmetic_relation'] == 'above') or early_repeat
            require(g.exact(emitted, candidate if wins else baseline),
                    'observed constructed threshold outcome: ' + c.name)
            classification = ('active-strict-threshold' if active else
                              'greedy-early-repeat' if early_repeat else 'inactive-depth-control')
        elif 'table_length' in h:
            require(g.exact(emitted, [1, h['repeat_length'], 1]),
                    'observed baseline-family repeat publication: ' + c.name)
            classification = 'published-repeat-initial-selection-not-inferred'
        else:
            selected = 0 if c.depth == 0 else 3
            require(g.exact(emitted, [h['query_deltas'][selected], h['placed_lengths'][selected],
                                     h['placed_offbases'][selected]]),
                    'observed successive-copy publication: ' + c.name)
            classification = 'published-first-copy' if c.depth == 0 else 'published-fourth-copy'
        results.append(dict(id=c.name, classification=classification, emitted=emitted,
            final_next_to_update=row['final_next_to_update'] - 2,
            final_entropy=row['final_entropy'], final_lazy_skipping=row['final_lazy_skipping'],
            first_table_after=row['first_table']['after_fnv1a64'],
            second_table_after=row['second_table']['after_fnv1a64']))
    require(len(results) == 96, 'complete score observation inventory')
    paired = {r['id']: r for r in results}
    for r in results:
        if '-chain-' in r['id']:
            require(g.exact(r['emitted'], paired[r['id'].replace('-chain-', '-row-')]['emitted']),
                    'constructed chain/row sequence equivalence')
    return results


def fixture():
    for name, digest in {'scripts/check_lazy_family_recipes.py': CHECKER_SHA,
                         'scripts/check_lazy_family_capture.py': AUDITOR_SHA}.items():
        require(sha(bounded(name, 1 << 20)) == digest, 'reviewed checker identity')
    import check_lazy_family_recipes as checker
    import check_lazy_family_capture as auditor
    g, cases, recipes = checker.inputs()
    raw = bounded(COLLECTION + '/capture.json', 1 << 20)
    require(sha(raw) == FIXTURE_SHA, 'immutable capture identity')
    accepted = g.decode_json(raw)
    require(g.exact(accepted['schema_version'], 1) and accepted['upstream_revision'] == g.REVISION
            and accepted['auditor_sha256'] == AUDITOR_SHA, 'capture schema and provenance')
    local = set(checker.PINS) | {checker.MANIFEST, 'scripts/check_lazy_family_recipes.py',
                               'scripts/capture_lazy_families.py', 'tests/upstream/manifest.json'}
    require(g.exact(accepted['source'], {n: sha(bounded(n, 2 << 20)) for n in sorted(local)}),
            'unchanged captured source closure')
    pack = bounded(COLLECTION + '/observations.ndjson', 64 << 20)
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
    require(g.exact({k: accepted['audit'][k] for k in ('commands', 'recipes',
            'direct_observations', 'fresh_contexts', 'negative_rejections', 'exact_tiny_controls')},
            dict(commands=1468, recipes=645, direct_observations=2580, fresh_contexts=12,
                 negative_rejections=110, exact_tiny_controls=138)), 'complete capture totals')
    scores = score_observations(g, cases, recipes, observed)
    require(g.exact(scores, accepted['score_observations']), 'frozen score observations')
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
    print(json.dumps({'direct_recipes': 645, 'stored_observation_groups': len(observed),
                      'pack_bytes': accepted['pack_bytes'], 'predecessor_controls': 173,
                      'tail_cases': 30, 'score_cases': 96, 'child_processes': 0}, sort_keys=True))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
