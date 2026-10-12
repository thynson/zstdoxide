#!/usr/bin/env python3
"""Source-only successor public recipes; preserve both historical input sets."""
from pathlib import Path
from dataclasses import replace
import collections
import csv
import hashlib
import io
import json

ROOT = Path(__file__).resolve().parents[2]
REVISION = 'f8745da6ff1ad1e7bab384bd1f9d742439278e99'
MAX_SOURCE = 4 << 20
MAX_FRAME = 5 << 20
MAX_TOTAL_SOURCE = 128 << 20
PINS = {'scripts/check_reference_admission_v3.py': 'af3818761663797bfd748c50a2f844dca4512d5d274ea367450bcb95f64681ee',
 'scripts/check_reference_fast_interop.py': 'd1b8ee20ae3542df2a1d25748988cd6cba4a5c8c2b57c0c685b632e92dd94553',
 'scripts/import_reference_fast_fuzz.py': 'cbbbc0d8b4c8709b3002c7d55d995ed61477fbfa42f2ee624952d0d4c814adbc',
 'scripts/oracles/double_fast_interop.py': '05eefcf492ce7dca0741d3348d165aca8a48e402aa63710d1c043775125705f3',
 'scripts/oracles/greedy_frame_floor_recipes.py': 'e30cb2c60762b333d6e4b510e9c9a5384c43a20e93209e89d52716f099bae2c4',
 'scripts/oracles/greedy_frames.py': '360bf5139b7e2a886e9d2359c1ea258317238bd1906cfe9d123be33df7a0ce44',
 'tests/fixtures/reference-admission-v3/README.md': '56603e97ef8b1b1021dc00ce0fa2e72635fc97df897c644f3a06b6097ce829aa',
 'tests/fixtures/reference-admission-v3/broader.tsv': '545ab96911821c87a0dafe2d8ad06b5bdc83de2da8ff08a7a9e69d791c4db24c',
 'tests/fixtures/reference-admission-v3/fuzz.tsv': 'e7c8b01438e2bb35cc66a81d42ad4fa2f02203d222c25600efc2205267dae611',
 'tests/fixtures/reference-admission-v3/interop.tsv': 'd1297efaeb9496515f41aaab46bc1a903dd652710234bbac4fbc7306463a7f24',
 'tests/fixtures/reference-admission-v3/loops.tsv': 'bd35c8a40cc7f0bb01dc7393f1d072ff1240e923ffabe26cea24733e613d410e',
 'tests/fixtures/reference-admission-v3/provenance.json': '7e3f6d14a546390c525b00e1578e5e83d38f4f3ad5e4f95a2e1b1fc32b5e785f',
 'tests/upstream/reference-fast-fuzz/observations.tsv': '77a868913331f7e9edd4a73a60634eebd97d83b5b57050b6945fbde5b317b4e5'}


def require(value, reason):
    if not value:
        raise ValueError(reason)


def sha(value):
    return hashlib.sha256(value).hexdigest()


def rows(name):
    return list(csv.DictReader(io.StringIO((ROOT / name).read_text()), delimiter='\t'))


def checked_inputs():
    for name, digest in PINS.items():
        path = ROOT / name
        require(path.is_file() and not path.is_symlink() and sha(path.read_bytes()) == digest,
                'input identity before import: ' + name)


def selected_greedy_status(size, controls):
    # Complete explicit positive W/C/H/S/M overrides on a known-size source.
    # ZSTD_adjustCParams_internal reduces W first, H to W+1 and C to W;
    # only then is W raised to ten. All other scalar values are already valid.
    # This predicts local admission, never a C applied-parameter observation.
    import check_reference_admission_v3 as policy
    require(type(size) is int and 0 <= size <= MAX_SOURCE, 'known bounded source')
    require(type(controls) is tuple and len(controls) == 9
            and all(type(n) is int for n in controls), 'exact request scalars')
    _, window, chain, table, search, minimum, target, strategy, _ = controls
    require(strategy == 3 and all(n > 0 for n in (window, chain, table, search, minimum)),
            'fully explicit Greedy dimensions')
    policy.classify([window, chain, table, search, minimum, target, strategy])
    source_log = max(6, (max(size, 1) - 1).bit_length())
    window = min(window, source_log)
    return policy.classify([max(10, window), min(chain, window), min(table, window + 1),
                            search, minimum, target, strategy])


def recipes():
    checked_inputs()
    from oracles import double_fast_interop as previous
    from oracles import greedy_frames as ordinary, greedy_frame_floor_recipes as floors
    import check_reference_fast_interop as original
    import import_reference_fast_fuzz as fuzz
    result = []
    successor = {r['case']: r for r in rows('tests/fixtures/reference-admission-v3/broader.tsv')}
    old = previous.recipes()
    require(len(old) == len(successor) == 159, 'complete historical broader set')
    for case in old:
        row = successor[case.name]
        require(row['source_sha256'] == sha(case.source) and int(row['source_size']) == len(case.source)
                and tuple(map(int, row['request'].split(','))) == case.request
                and row['v2'] == case.expected, 'historical broader input/request/status')
        result.append(replace(case, expected=row['v3']))
    successor = {r['case']: r for r in rows('tests/fixtures/reference-admission-v3/interop.tsv')}
    old = original.recipes()
    require(len(old) == len(successor) == 92, 'complete original interop set')
    for case in old:
        row = successor[case['id']]
        data = original.source(case)
        controls = (case['level'], *case['overrides'], case['hint'])
        require(row['source_sha256'] == sha(data) and int(row['source_size']) == len(data)
                and int(row['level']) == case['level'] and int(row['hint']) == case['hint']
                and tuple(map(int, row['overrides'].split(','))) == tuple(case['overrides']),
                'original source/request identity')
        result.append(previous.Recipe('historical-' + case['id'], data, controls, row['v3'],
                      'Unchanged original92 public input/request under frozen v3 admission; historical C/Rust evidence remains immutable.'))
    for case in ordinary.recipes() + floors.recipes():
        controls = previous.request(level=4, window=case.window_log, chain=case.chain_log,
                    table=case.hash_log, search=case.search_log, minimum=case.min_match,
                    target=case.target, strategy=3)
        result.append(previous.Recipe('greedy-' + case.name, case.source, controls,
                    selected_greedy_status(len(case.source), controls),
                    'Frozen ordinary Greedy source in one public call; original schedule/applied parameters are separate. Actual size may change selected family.'))
    selector = next(r for r in rows('tests/upstream/reference-fast-fuzz/observations.tsv') if r['case'] == 'unsupported-57')
    selected = next(c for c in fuzz.cases() if c['case'] == selector['case'])
    require(sha(selected['input']) == selector['input_sha256'] and selector['branch'] == '0',
            'original Greedy selector identity')
    result.append(previous.Recipe('selector-unsupported-57', selected['input'][:int(selector['prefix_size'])],
                  previous.request(level=int(selector['level'])), 'success',
                  'Exact newly admitted simple_round_trip.c source prefix and level from the frozen original C selector.'))

    def add(name, data, **values):
        controls = previous.request(level=4, strategy=3, **values)
        result.append(previous.Recipe(name, data, controls, selected_greedy_status(len(data), controls)))

    def periodic(size):
        unit = bytes(range(251))
        return (unit * ((size + 250) // 251))[:size]

    for size in (0, 16384, 16385):
        add(f'greedy-adjust-{size}', periodic(size), window=21, chain=30, table=19, search=8, minimum=4)
    add('greedy-adjust-hint', periodic(16385), window=21, chain=30, table=19,
        search=8, minimum=4, hint=(1 << 31) - 1)
    for search in (1, 4, 5, 6, 30):
        add(f'greedy-row-budget-{search}', periodic(65536), window=16, chain=6, table=12,
            search=search, minimum=4)
    for branch, size, window in (('chain', 8192, 14), ('row', 65536, 16)):
        for minimum in range(3, 8):
            add(f'greedy-{branch}-key-{minimum}', periodic(size), window=window,
                chain=12, table=12, search=4, minimum=minimum)
        for target in (0, 4, 131072):
            add(f'greedy-{branch}-target-{target}', periodic(size), window=window,
                chain=12, table=12, search=4, minimum=4, target=target)
    large = periodic((1 << 21) + 1)
    for chain, search in ((6, 6), (30, 6), (30, 30)):
        add(f'greedy-maximum-c{chain}-s{search}', large, window=21, chain=chain,
            table=19, search=search, minimum=4)
    for window, table, search in ((22, 19, 6), (21, 20, 6), (14, 15, 9)):
        add(f'greedy-resource-w{window}-h{table}-s{search}', large, window=window,
            chain=18, table=table, search=search, minimum=4)
    result.append(previous.Recipe('greedy-unsupported-before-resource', large,
                  previous.request(level=4, window=31, chain=30, table=30, search=30,
                                   minimum=4, strategy=4), 'unsupported'))
    for case in result:
        case.metadata()
    require(len(result) == len({r.name for r in result}) == 422, 'unique successor count')
    require(sum(len(r.source) for r in result) <= MAX_TOTAL_SOURCE, 'aggregate source cap')
    return result


def manifest():
    cases = recipes()
    from oracles import double_fast_interop as previous
    equal = previous.manifest()['expected_rust_frame_equivalence']
    equal += [['greedy-adjust-16385', 'greedy-adjust-hint'],
              ['greedy-row-budget-6', 'greedy-row-budget-30'],
              ['greedy-maximum-c6-s6', 'greedy-maximum-c30-s6', 'greedy-maximum-c30-s30']]
    for branch in ('chain', 'row'):
        equal += [[f'greedy-{branch}-key-3', f'greedy-{branch}-key-4'],
                  [f'greedy-{branch}-key-6', f'greedy-{branch}-key-7'],
                  [f'greedy-{branch}-target-{target}' for target in (0, 4, 131072)]]
    by_name = {r.name: r for r in cases}
    for group in equal:
        require(all(by_name[n].expected == 'success' for n in group), 'equivalence requires admission')
        require(len({sha(by_name[n].source) for n in group}) == 1, 'equivalence source identity')
    return dict(schema_version=3, upstream_revision=REVISION,
                status='Source-only pre-execution inputs and hypotheses.',
                source_limit=MAX_SOURCE, frame_limit=MAX_FRAME, source_bytes=sum(len(r.source) for r in cases),
                recipe_count=len(cases), classifications=dict(collections.Counter(r.expected for r in cases)),
                input_sha256=PINS, request_fields=['level', 'window', 'chain', 'hash', 'search', 'minimum', 'target', 'strategy', 'hint'],
                recipes=[r.metadata() for r in cases], expected_rust_frame_equivalence=equal,
                families={'broader_historical':159, 'original_historical':92, 'ordinary_greedy_sources':138,
                          'original_greedy_selector':1, 'additional_controls':32},
                qualification='New public calls; no C scalar or entropy-byte prediction. Both historical sets retain all source/request bytes. Ordinary source names do not imply retained schedules or branches after public adjustment.')
