#!/usr/bin/env python3
"""Freeze successor Rust admission over immutable Module12 observations; offline only."""
from pathlib import Path
import argparse
import collections
import csv
import hashlib
import io
import json

ROOT = Path(__file__).resolve().parents[1]
DESTINATION = ROOT / 'tests/fixtures/reference-admission-v2'
PINS = {
    'scripts/import_reference_fast_fuzz.py': 'cbbbc0d8b4c8709b3002c7d55d995ed61477fbfa42f2ee624952d0d4c814adbc',
    'scripts/check_reference_fast_interop.py': 'd1b8ee20ae3542df2a1d25748988cd6cba4a5c8c2b57c0c685b632e92dd94553',
    'docs/evidence/2026-10-09-reference-fast-interop.json': '648b0c24c2ea716be31bb38a2d3fc9316ecf0a17e912b814af157e9a1bb47d27',
    'tests/upstream/parameters/presets.tsv': '4e188cc16826c6665dcd440ae9d153484aeb4e561321a089be2aec260667881d',
    'tests/upstream/reference-fast-fuzz/observations.tsv': '77a868913331f7e9edd4a73a60634eebd97d83b5b57050b6945fbde5b317b4e5',
}
# Filled from the reviewed source-only projection before any public API changes.
DATA_SHA256 = {'fuzz.tsv': '7b2fbce793df15e2185b65bad272e9e6428a614fdb00905f949bfeb8d45bbc13', 'interop.tsv': '6c1b9c77329cba64893738b92304d5e0020c4bc0a8ff08e6ea2da9d27d0c516c', 'loops.tsv': 'c5c2bed1fe2dcf8631d89eecf98510cab81b9c115d365e6a5dadef216245ce6d'}


def require(value, reason):
    if not value:
        raise ValueError(reason)


def sha(value):
    return hashlib.sha256(value).hexdigest()


def classify(values, version):
    require(type(values) is list and len(values) == 7 and all(type(x) is int for x in values), 'exact integer tuple')
    limits = [(10, 31), (6, 30), (6, 30), (1, 30), (3, 7), (0, 131072), (1, 9)]
    require(all(a <= x <= b for x, (a, b) in zip(values, limits)), 'valid scalar tuple')
    require(type(version) is int and version in (1, 2), 'policy version')
    window, chain, table, _, _, _, strategy = values
    if strategy != 1 and (version == 1 or strategy != 2):
        return 'unsupported'
    if window > (20 if strategy == 1 else 21) or table > 18 or (strategy == 2 and chain > 18):
        return 'resource'
    return 'success'


def tsv(fields, rows):
    out = io.StringIO(newline='')
    writer = csv.writer(out, delimiter='\t', lineterminator='\n')
    writer.writerow(fields)
    writer.writerows(rows)
    return out.getvalue().encode('ascii')


def encoded(value):
    return (json.dumps(value, sort_keys=True, indent=2) + '\n').encode()


def projection():
    for name, digest in PINS.items():
        p = ROOT / name
        require(p.is_file() and not p.is_symlink() and sha(p.read_bytes()) == digest, 'frozen input before import: ' + name)
    import import_reference_fast_fuzz as fuzz
    import check_reference_fast_interop as interop
    data, historical_fuzz = fuzz.offline()
    presets = {}
    for r in csv.DictReader(io.StringIO((ROOT / 'tests/upstream/parameters/presets.tsv').read_text()), delimiter='\t'):
        key = (int(r['tier']), int(r['row']))
        require(key not in presets, 'unique preset')
        presets[key] = [int(r[k]) for k in ('window_log', 'chain_log', 'hash_log', 'search_log', 'min_match', 'target_length', 'strategy')]

    def preset(size, level):
        tier = 3 if size <= 16384 else 2 if size <= 131072 else 1 if size <= 262144 else 0
        row = 0 if level < 0 else 3 if level == 0 else level
        return tier, row, presets[tier, row]

    # These are pre-source-adjustment C presets, not applied scalar tuples.
    # In this fixed corpus every supported preset already fits its algorithm's
    # caps. Known-source adjustment only reduces those dimensions, so it cannot
    # turn these successes into resource errors or change their strategies.
    fuzz_rows = []
    for row in csv.DictReader(io.StringIO(data.decode('ascii')), delimiter='\t'):
        size = int(row['prefix_size'])
        if int(row['branch']):
            fuzz_rows.append([row['case'], row['input_sha256'], size, '-', 'advanced', 'advanced', '-', '-', '-'])
            continue
        level = int(row['level'])
        tier, index, values = preset(size, level)
        old, new = classify(values, 1), classify(values, 2)
        require('resource' not in (old, new), 'fixed selector presets fit operational caps')
        fuzz_rows.append([row['case'], row['input_sha256'], size, level, old, new, tier, index, values[6]])
    old_fuzz = dict(collections.Counter(r[4] for r in fuzz_rows))
    new_fuzz = dict(collections.Counter(r[5] for r in fuzz_rows))
    require(old_fuzz == {'success': 57, 'unsupported': 20, 'advanced': 4}, 'historical selector classes')
    require(new_fuzz == {'success': 61, 'unsupported': 16, 'advanced': 4}, 'successor selector classes')
    require(historical_fuzz['expected_rust_classifications'] == {'admitted': 57, 'unsupported': 20, 'advanced': 4}, 'original manifest preserved')

    history = fuzz.decode_json((ROOT / 'docs/evidence/2026-10-09-reference-fast-interop.json').read_bytes())
    recipes = interop.recipes()
    require(len(history['cases']) == len(recipes) == 92, 'historical interop coverage')
    interop_rows = []
    for recipe, record in zip(recipes, history['cases'], strict=True):
        require(recipe == record['recipe'], 'original interop recipe identity')
        require(sha(interop.source(recipe)) == record['source_sha256'], 'original interop source identity')
        safe, all_features = record['rust']['safe'], record['rust']['all']
        require(safe == all_features, 'historical safe/all identity')
        values = safe['resolved']
        old, new = classify(values, 1), classify(values, 2)
        require(old == recipe['expected'] == safe['status'], 'historical applied classification')
        additive = old != 'success' and new == 'success'
        if old != 'success':
            require(record['c'] == {'status': 'not-run-for-rejected-Rust-profile'}, 'never relabel old missing C runs')
        interop_rows.append([recipe['id'], record['source_sha256'], recipe['size'], recipe['level'], recipe['hint'],
                             ','.join(map(str, recipe['overrides'])), ','.join(map(str, values)), old, new, int(additive)])
    old_interop = dict(collections.Counter(r[7] for r in interop_rows))
    new_interop = dict(collections.Counter(r[8] for r in interop_rows))
    require(old_interop == {'success': 79, 'unsupported': 11, 'resource': 2}, 'historical interop classes')
    require(new_interop == {'success': 85, 'unsupported': 4, 'resource': 3}, 'successor interop classes')
    require(sum(r[9] for r in interop_rows) == 6, 'six additive C obligations')

    loops = []
    for family, size, levels in [('empty', 0, range(-1, 23)), ('initial_match', 30, range(1, 20))]:
        for level in levels:
            tier, index, values = preset(size, level)
            loops.append([family, size, level, classify(values, 1), classify(values, 2), tier, index, values[6]])
    files = {
        'fuzz.tsv': tsv(['case', 'input_sha256', 'prefix_size', 'level', 'historical', 'successor', 'preset_tier', 'preset_row', 'preset_strategy'], fuzz_rows),
        'interop.tsv': tsv(['case', 'source_sha256', 'source_size', 'level', 'hint', 'overrides', 'historical_resolved_tuple', 'historical', 'successor', 'additive_c_required'], interop_rows),
        'loops.tsv': tsv(['family', 'source_size', 'level', 'historical', 'successor', 'preset_tier', 'preset_row', 'preset_strategy'], loops),
    }
    require({n: sha(b) for n, b in files.items()} == DATA_SHA256, 'fixed successor data identity')
    metadata = {
        'schema_version': 2, 'upstream_revision': fuzz.REVISION,
        'generator': str(Path(__file__).relative_to(ROOT)), 'generator_sha256': sha(Path(__file__).read_bytes()),
        'input_sha256': PINS, 'historical_selector': old_fuzz, 'successor_selector': new_fuzz,
        'historical_interop': old_interop, 'successor_interop': new_interop,
        'loop_classifications': {name: {version: dict(collections.Counter(r[column] for r in loops if r[0] == name))
                                      for version, column in [('historical', 3), ('successor', 4)]}
                                 for name in ('empty', 'initial_match')},
        'additive_c_required': [r[0] for r in interop_rows if r[9]],
        'newly_admitted_selectors': [r[0] for r in fuzz_rows if r[4] != r[5]],
        'changed_interop': [{'case': r[0], 'before': r[7], 'after': r[8]} for r in interop_rows if r[7] != r[8]],
        'algorithm_caps': {'Fast': {'window': 20, 'hash': 18, 'chain_consumed': False},
                           'DoubleFast': {'window': 21, 'hash': 18, 'chain': 18}},
        'fixtures': [{'path': n, 'bytes': len(b), 'sha256': sha(b)} for n, b in sorted(files.items())],
        'qualification': 'Source-only successor expectations before public DoubleFast execution. Selector tuples and C presets remain original; pre-adjustment presets are not applied scalar observations. Interop tuples are historical Rust resolution from the archived Module12 campaign. Six new positive C cases are obligations, not executions; no codec or child process runs.',
    }
    files['provenance.json'] = encoded(metadata)
    return files


def verify_files(destination, files):
    require({p.name for p in destination.iterdir()} == set(files) | {'README.md'}, 'exact successor paths')
    for name, data in files.items():
        p = destination / name
        require(p.is_file() and not p.is_symlink() and p.read_bytes() == data, 'successor artifact: ' + name)
    return files


def check(destination=DESTINATION):
    return verify_files(destination, projection())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument('--write', action='store_true')
    action.add_argument('--check', action='store_true')
    args = parser.parse_args()
    if args.write:
        DESTINATION.mkdir(parents=True, exist_ok=True)
        for name, data in projection().items():
            (DESTINATION / name).write_bytes(data)
    else:
        check()
    print('Verified successor policy: selectors 61/16/4; interop 85/4/3; six additive C obligations. No codec execution.')


if __name__ == '__main__':
    main()
