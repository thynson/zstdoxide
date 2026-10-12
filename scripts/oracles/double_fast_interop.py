#!/usr/bin/env python3
"""Independent public-frame input recipes; no codec execution or predicted bytes."""
from dataclasses import dataclass
from pathlib import Path
import csv
import hashlib
import io
import re

ROOT = Path(__file__).resolve().parents[2]
REVISION = 'f8745da6ff1ad1e7bab384bd1f9d742439278e99'
MAX_SOURCE = 4 << 20
MAX_FRAME = 5 << 20
MAX_TOTAL_SOURCE = 128 << 20
PINS = {
    'scripts/oracles/double_fast_frames.py': 'ac10609d7fe7c0f4983899238dc8a852ca437e35aff42b317599a0286c8ea656',
    'scripts/import_reference_fast_fuzz.py': 'cbbbc0d8b4c8709b3002c7d55d995ed61477fbfa42f2ee624952d0d4c814adbc',
    'tests/upstream/reference-fast-fuzz/observations.tsv': '77a868913331f7e9edd4a73a60634eebd97d83b5b57050b6945fbde5b317b4e5',
    'tests/fixtures/reference-admission-v2/loops.tsv': 'c5c2bed1fe2dcf8631d89eecf98510cab81b9c115d365e6a5dadef216245ce6d',
}


def require(value, reason):
    if not value:
        raise ValueError(reason)


def sha(data):
    return hashlib.sha256(data).hexdigest()


@dataclass(frozen=True)
class Recipe:
    name: str
    source: bytes
    # level, window, chain, hash, search, minimum, target, strategy, source hint
    request: tuple
    expected: str = 'success'
    origin: str = 'Independent deterministic public-profile input.'

    def metadata(self):
        require(type(self.name) is str and re.fullmatch('[a-z][a-z0-9_-]{0,79}', self.name), 'recipe name')
        require(type(self.source) is bytes and len(self.source) <= MAX_SOURCE, 'source extent/type')
        require(type(self.request) is tuple and len(self.request) == 9
                and all(type(v) is int for v in self.request), 'integer request tuple')
        level, window, chain, table, search, minimum, target, strategy, hint = self.request
        require(-(1 << 31) <= level < 1 << 31, 'signed level')
        for value, lower, upper in [(window, 10, 31), (chain, 6, 30), (table, 6, 30),
                                     (search, 1, 30), (minimum, 3, 7), (target, 0, 131072),
                                     (strategy, 1, 9), (hint, 0, (1 << 31) - 1)]:
            require(value == 0 or lower <= value <= upper, 'override domain')
        require(self.expected in ('success', 'unsupported', 'resource'), 'expected admission')
        require(type(self.origin) is str and 0 < len(self.origin) <= 256, 'origin text')
        return dict(id=self.name, source_bytes=len(self.source), source_sha256=sha(self.source),
                    request=list(self.request), expected=self.expected, origin=self.origin)


def request(level=3, window=0, chain=0, table=0, search=0, minimum=0, target=0, strategy=0, hint=0):
    return (level, window, chain, table, search, minimum, target, strategy, hint)


def recipes():
    for name, digest in PINS.items():
        path = ROOT / name
        require(path.is_file() and not path.is_symlink() and sha(path.read_bytes()) == digest,
                'input identity before import: ' + name)
    from oracles import double_fast_frames as ordinary
    import import_reference_fast_fuzz as fuzz
    rows = []

    def add(name, source, controls=None, expected='success', origin=None):
        row = Recipe(name, source, request() if controls is None else controls, expected,
                     origin or 'Independent deterministic public-profile input.')
        row.metadata()
        rows.append(row)

    for case in ordinary.recipes():
        add('ordinary-' + case.name, case.source,
            request(window=case.window_log, chain=case.short_log, table=case.long_log,
                    search=case.search_log, minimum=case.min_match, target=case.target, strategy=2),
            origin='Frozen ordinary recipe source, now one public known-size call. '
                   'Its original schedule and internal applied tuple are not reused as public observations.')
    pattern = bytes(range(251))
    def periodic(size):
        return (pattern * ((size + 250) // 251))[:size]

    for size in (0, 6, 7, 16384, 16385, 131072, 131073, 262144, 262145, 2097153):
        data = periodic(size)
        for level in (0, 2, 3, 4):
            status = 'unsupported' if level == 4 and (size <= 16384 or 131072 < size <= 262144) else 'success'
            add(f'tier-{size}-level-{level}', data, request(level=level), status,
                'Pinned clevels.h four source tiers; level0/3 equivalence and level2/4 strategy boundaries.')

    data = ordinary.repeated(0x19283746, 257, 262145)
    for table, chain in ((6, 18), (18, 6)):
        for minimum in (3, 4, 7):
            add(f'unequal-h{table}-c{chain}-m{minimum}', data,
                request(window=19, chain=chain, table=table, minimum=minimum, strategy=2))
    for search, target in ((1, 0), (30, 0), (1, 1), (30, 131072)):
        add(f'control-s{search}-t{target}', data,
            request(level=-7, window=19, chain=14, table=16, search=search,
                    minimum=5, target=target, strategy=2))
    for hint in (1, 16384, (1 << 31) - 1):
        add(f'hint-{hint}', data, request(hint=hint))
    add('hint-baseline', data)
    large = periodic((1 << 21) + 1)
    for window, chain, table, strategy, status in ((21, 18, 18, 2, 'success'),
            (22, 18, 18, 2, 'resource'), (21, 19, 18, 2, 'resource'),
            (21, 18, 19, 2, 'resource'), (21, 18, 18, 1, 'resource'),
            (22, 19, 19, 3, 'unsupported')):
        add(f'cap-w{window}-c{chain}-h{table}-s{strategy}', large,
            request(window=window, chain=chain, table=table, minimum=4, strategy=strategy), status)
    add('maximum-source', periodic(MAX_SOURCE),
        request(window=21, chain=18, table=18, minimum=4, strategy=2))

    # These exact original fuzzer loops are frozen separately from synthetic tiers.
    for row in csv.DictReader(io.StringIO((ROOT / 'tests/fixtures/reference-admission-v2/loops.tsv').read_text()), delimiter='\t'):
        data = b'' if row['family'] == 'empty' else bytes(range(20)) + bytes(range(10))
        require(len(data) == int(row['source_size']), 'original loop source')
        add(f"fuzzer-{row['family']}-level-{row['level']}", data,
            request(level=int(row['level'])), row['successor'],
            'Original fuzzer.c:1554/1637 one-shot subset; no NULL, reusable-context or caller-output claim.')
    captured, _ = fuzz.offline()
    observations = {r['case']: r for r in csv.DictReader(io.StringIO(captured.decode()), delimiter='\t')}
    for case in fuzz.cases():
        if case['case'] not in ('tier-44', 'tier-49', 'unsupported-55', 'unsupported-56'):
            continue
        row = observations[case['case']]
        require(sha(case['input']) == row['input_sha256'] and row['branch'] == '0', 'original selector identity')
        add('selector-' + case['case'], case['input'][:int(row['prefix_size'])],
            request(level=int(row['level'])),
            origin='Exact newly admitted simple_round_trip.c source prefix and selected level from frozen C producer.')
    require(len(rows) == len({r.name for r in rows}) == 159, 'fixed unique recipe count')
    require(sum(len(r.source) for r in rows) <= MAX_TOTAL_SOURCE, 'aggregate source bound')
    return rows


def manifest():
    rows = [r.metadata() for r in recipes()]
    equal = [[f'tier-{n}-level-0', f'tier-{n}-level-3']
             for n in (0, 6, 7, 16384, 16385, 131072, 131073, 262144, 262145, 2097153)]
    equal += [[f'unequal-h{h}-c{c}-m3', f'unequal-h{h}-c{c}-m4'] for h, c in ((6, 18), (18, 6))]
    equal += [['control-s1-t0', 'control-s30-t0', 'control-s1-t1', 'control-s30-t131072'],
              ['hint-baseline', 'hint-1', 'hint-16384', 'hint-2147483647'],
              ['ordinary-literal_tree', 'ordinary-literal_target4', 'ordinary-literal_target_max', 'ordinary-literal_search29'],
              ['ordinary-short_width3', 'ordinary-short_width4']]
    by_name = {r['id']: r for r in rows}
    for group in equal:
        require(all(by_name[n]['expected'] == 'success' for n in group), 'equivalence requires admission')
        require(len({by_name[n]['source_sha256'] for n in group}) == 1, 'equivalence source identity')
    return dict(schema_version=1, upstream_revision=REVISION, status='Source-only pre-execution inputs and hypotheses.',
                source_limit=MAX_SOURCE, frame_limit=MAX_FRAME, source_bytes=sum(r['source_bytes'] for r in rows),
                recipe_count=len(rows), classifications={s: sum(r['expected'] == s for r in rows)
                    for s in ('success', 'unsupported', 'resource')}, input_sha256=PINS,
                request_fields=['level', 'window', 'chain', 'hash', 'search', 'minimum', 'target', 'strategy', 'hint'],
                recipes=rows, expected_rust_frame_equivalence=equal,
                qualification='No resolved scalar or wire-byte prediction. Complete public frames replace no existing internal-state/schedule observations.')
