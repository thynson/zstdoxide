#!/usr/bin/env python3
"""Source-only Lazy-family public inputs; preserve every historical request."""
from pathlib import Path
from dataclasses import replace
import collections
import csv
import hashlib
import io

ROOT = Path(__file__).resolve().parents[2]
REVISION = 'f8745da6ff1ad1e7bab384bd1f9d742439278e99'
MAX_SOURCE = 4 << 20
MAX_FRAME = 5 << 20
MAX_TOTAL_SOURCE = 256 << 20
PINS = {
  "scripts/check_reference_admission_v4.py": "0589976b7ebdc12d132e8e9c1cc785d9952fae0ec158a3a61ca79d39f4785091",
  "scripts/oracles/greedy_interop.py": "c1eac37d63179fc02dcb2c6ac21d46b3810eb96763c782b59799973db07e6c7a",
  "scripts/oracles/lazy_frames.py": "7fd553c0c39270e4f12828c500fd33485eb5c6b3701062778816285511390894",
  "scripts/oracles/double_fast_interop.py": "05eefcf492ce7dca0741d3348d165aca8a48e402aa63710d1c043775125705f3",
  "scripts/import_reference_fast_fuzz.py": "cbbbc0d8b4c8709b3002c7d55d995ed61477fbfa42f2ee624952d0d4c814adbc",
  "tests/upstream/parameters/presets.tsv": "4e188cc16826c6665dcd440ae9d153484aeb4e561321a089be2aec260667881d",
  "docs/reference-lazy-interop-protocol.md": "6d6ad675e7fbca8954772baba42d1efb9a58456014f62a5ca53fb2767e9e2914",
  "tests/fixtures/reference-admission-v4/README.md": "8ab0c5337362dc2f33308f842376f692811d7f7c38e23f0f8f2f831bd952e25c",
  "tests/fixtures/reference-admission-v4/broader.tsv": "b859aaaa9dcc7ccafd3cad3a859524870e9d570fcef86bf8511812471122f7ba",
  "tests/fixtures/reference-admission-v4/fuzz.tsv": "d82a887c2e9aff5848d2aa9385ced786b91422b239d4817a233b87b97e888f45",
  "tests/fixtures/reference-admission-v4/greedy.tsv": "3fea6e0c916a1fd8d391f3473fb490db07dc8d746559d57b01e958da7063c765",
  "tests/fixtures/reference-admission-v4/interop.tsv": "175ee2f09d3646bdf1e27083686eac4d7bb38b593c9dfc1daeed750f46d41110",
  "tests/fixtures/reference-admission-v4/loops.tsv": "84a06c0ff6794d2e5c5eccc8fb160252dbd72511c6634985a18fa06e1ba92d85",
  "tests/fixtures/reference-admission-v4/provenance.json": "703418ab6cb62f9cfbbf2c4ac8a8a3ca6524302aa74fc4bbb5a5722a95749460"
}


def require(value, reason):
    if not value:
        raise ValueError(reason)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def rows(name):
    return list(csv.DictReader(io.StringIO((ROOT / name).read_text()), delimiter='\t'))


def checked_inputs():
    for name, digest in PINS.items():
        p = ROOT / name
        require(p.is_file() and not p.is_symlink() and sha(p.read_bytes()) == digest,
                'source identity before import: ' + name)


def selected_status(size, controls):
    import check_reference_admission_v4 as policy
    require(type(size) is int and 0 <= size <= MAX_SOURCE, 'bounded actual source')
    require(type(controls) is tuple and len(controls) == 9
            and all(type(n) is int for n in controls), 'exact request')
    level, window, chain, table, search, minimum, target, strategy, hint = controls
    require(1 <= level <= 22 and 0 <= hint < 1 << 31, 'bounded positive-level recipe')
    tier = 3 if size <= 16384 else 2 if size <= 131072 else 1 if size <= 262144 else 0
    preset = next(r for r in rows('tests/upstream/parameters/presets.tsv')
                  if int(r['tier']) == tier and int(r['row']) == level)
    requested = [window, chain, table, search, minimum, target, strategy]
    values = [n or int(preset[k]) for n, k in zip(requested,
              ('window_log', 'chain_log', 'hash_log', 'search_log', 'min_match', 'target_length', 'strategy'))]
    policy.classify(values)  # Reject scalar domain errors before adjustment.
    if values[6] >= 6:
        return 'unsupported'
    require(values[6] in (3, 4, 5), 'new recipe uses the shared lazy family')
    source_log = max(6, (max(size, 1) - 1).bit_length())
    adjusted = min(values[0], source_log)
    values[0] = max(10, adjusted)
    values[1] = min(values[1], adjusted)
    values[2] = min(values[2], adjusted + 1)
    return policy.classify(values)


def recipes():
    checked_inputs()
    from oracles import greedy_interop as prior, lazy_frames as ordinary, double_fast_interop as common
    import import_reference_fast_fuzz as fuzz
    old = prior.recipes()
    mapping = rows('tests/fixtures/reference-admission-v4/greedy.tsv')
    require(len(old) == len(mapping) == 422, 'complete predecessor set')
    result = []
    for case, row in zip(old, mapping, strict=True):
        require(case.name == row['case'] and sha(case.source) == row['source_sha256']
                and len(case.source) == int(row['source_size'])
                and case.request == tuple(map(int, row['request'].split(',')))
                and case.expected == row['v3'], 'unchanged predecessor bytes/scalars')
        result.append(replace(case, expected=row['v4']))
    for case in ordinary.recipes():
        if case.depth == 0:
            continue
        controls = common.request(level=6, window=case.window_log, chain=case.chain_log,
                                  table=case.hash_log, search=case.search_log, minimum=case.min_match,
                                  target=case.target, strategy=3 + case.depth)
        result.append(common.Recipe('lazy-' + case.name, case.source, controls,
                                    selected_status(len(case.source), controls),
                                    'Frozen ordinary Lazy source through public known-size resolution; prior schedules and branches remain separate.'))
    selectors = {r['case']: r for r in rows('tests/fixtures/reference-admission-v4/fuzz.tsv')
                 if r['v3'] != r['v4']}
    require(set(selectors) == {f'unsupported-{i}' for i in range(58, 62)}, 'four original additions')
    for case in fuzz.cases():
        if case['case'] not in selectors:
            continue
        row = selectors[case['case']]
        require(sha(case['input']) == row['input_sha256'], 'original selector source')
        result.append(common.Recipe('selector-' + case['case'], case['input'][:int(row['prefix_size'])],
                                    common.request(level=int(row['level'])), row['v4'],
                                    'Unchanged original simple-round-trip selector prefix and level.'))
    require(len(old[-32:]) == 32 and all(c.name.startswith('greedy-') for c in old[-32:]), 'fixed public boundary controls')
    for depth in (1, 2):
        for case in old[-32:]:
            controls = (*case.request[:7], depth + 3, case.request[8])
            result.append(common.Recipe(f'lazy{depth}-' + case.name, case.source, controls,
                                        selected_status(len(case.source), controls),
                                        'Unchanged Greedy boundary source/controls except explicit Lazy-family strategy.'))
    unit = bytes(range(251))
    for size in (0, 16384, 16385, 131072, 131073, 262144, 262145):
        data = (unit * ((size + 250) // 251))[:size]
        for level in range(5, 13):
            controls = common.request(level=level)
            result.append(common.Recipe(f'lazy-level-{level}-size-{size}', data, controls,
                                        selected_status(size, controls), 'Unmodified level request at exact preset boundaries.'))
    for case in result:
        case.metadata()
    require(len(result) == len({c.name for c in result}) == 886, 'unique complete successor count')
    require(sum(len(c.source) for c in result) <= MAX_TOTAL_SOURCE, 'bounded aggregate source')
    return result


def manifest():
    from oracles import greedy_interop as prior
    cases = recipes()
    previous = prior.manifest()['expected_rust_frame_equivalence']
    equal = list(previous)
    groups = [g for g in previous if all(n.startswith('greedy-') for n in g)]
    require(len(groups) == 9, 'existing Greedy boundary equivalences')
    for depth in (1, 2):
        equal += [[f'lazy{depth}-' + n for n in group] for group in groups]
    by_name = {c.name: c for c in cases}
    for group in equal:
        require(all(by_name[n].expected == 'success' for n in group), 'admitted equivalence')
        require(len({sha(by_name[n].source) for n in group}) == 1, 'identical equivalence sources')
    return dict(schema_version=4, upstream_revision=REVISION, status='Source-only pre-execution inputs and hypotheses.',
                source_limit=MAX_SOURCE, frame_limit=MAX_FRAME, aggregate_source_limit=MAX_TOTAL_SOURCE,
                source_bytes=sum(len(c.source) for c in cases), recipe_count=len(cases),
                classifications=dict(collections.Counter(c.expected for c in cases)), input_sha256=PINS,
                request_fields=['level', 'window', 'chain', 'hash', 'search', 'minimum', 'target', 'strategy', 'hint'],
                recipes=[c.metadata() for c in cases], expected_rust_frame_equivalence=equal,
                families=dict(historical=422, ordinary_lazy=340, original_selectors=4, boundary_controls=64, numeric_boundaries=56),
                qualification='New public calls. No C scalar or entropy-byte prediction; original sources/requests and rejected historical records remain unchanged.')
