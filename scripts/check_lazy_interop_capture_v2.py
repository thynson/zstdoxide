#!/usr/bin/env python3
"""Add independent new-request scalar reconstruction to immutable v1 replay."""
from pathlib import Path
import argparse
import csv
import hashlib
import io
import json

ROOT = Path(__file__).resolve().parents[1]
PRIOR_SHA256 = '1bb04d8009dbdfa3ff3c353d7aa1e29dc9f7bfd7b1d5cede6bf3854a4529bce5'
PRESET_SHA256 = '4e188cc16826c6665dcd440ae9d153484aeb4e561321a089be2aec260667881d'


def require(value, reason):
    if not value:
        raise ValueError(reason)


def presets():
    path = ROOT / 'tests/upstream/parameters/presets.tsv'
    require(path.is_file() and not path.is_symlink()
            and hashlib.sha256(path.read_bytes()).hexdigest() == PRESET_SHA256,
            'original C preset identity')
    return {(int(r['tier']), int(r['row'])):
            [int(r[k]) for k in ('window_log', 'chain_log', 'hash_log', 'search_log',
                                 'min_match', 'target_length', 'strategy')]
            for r in csv.DictReader(io.StringIO(path.read_text()), delimiter='\t')}


def resolved_new_request(case, table):
    """Only frozen positive-level, known-size, no-dictionary successor inputs.

    Reconstruct two successive adjustments around explicit overrides. This is
    an independent expectation from pinned C presets, not an observed C tuple.
    """
    require(type(case.source) is bytes, 'owned source bytes')
    size = len(case.source)
    require(0 <= size <= 4 << 20 and type(case.request) is tuple
            and len(case.request) == 9 and all(type(n) is int for n in case.request),
            'bounded exact successor request')
    level, *controls = case.request
    require(1 <= level <= 22 and 0 <= controls[-1] < 1 << 31, 'positive level / hint')
    tier = sum(size <= boundary for boundary in (16384, 131072, 262144))
    value = table[tier, level].copy()
    limits = [(10, 31), (6, 30), (6, 30), (1, 30), (3, 7), (0, 131072), (1, 9)]
    for n, (lo, hi) in zip(controls[:-1], limits, strict=True):
        require(n == 0 or lo <= n <= hi, 'override scalar domain')

    def adjusted(v):
        source_log = max(6, (max(size, 1) - 1).bit_length())
        window = min(v[0], source_log)
        v[2] = min(v[2], window + 1)
        v[1] = min(v[1], window + int(v[6] >= 6))
        v[0] = max(10, window)
        if 3 <= v[6] <= 5:
            v[2] = min(v[2], 24 + min(6, max(4, v[3])))
        return v

    value = adjusted(value)
    value = [n if n else default for n, default in zip(controls[:-1], value, strict=True)]
    return adjusted(value)


def check(directory):
    path = ROOT / 'scripts/check_lazy_interop_capture.py'
    require(path.is_file() and not path.is_symlink()
            and hashlib.sha256(path.read_bytes()).hexdigest() == PRIOR_SHA256,
            'immutable complete v1 auditor')
    import check_lazy_interop_capture as prior
    totals = prior.check(directory)
    from oracles import lazy_interop as generator
    cases = generator.recipes()
    table = presets()
    evidence = json.loads((directory / 'evidence.json').read_bytes())
    require(len(cases) == len(evidence['cases']) == 886, 'complete successor set')
    for case, observed in zip(cases[422:], evidence['cases'][422:], strict=True):
        expected = resolved_new_request(case, table)
        for mode in ('safe', 'all'):
            require(observed['rust'][mode]['resolved'] == expected,
                    'independent successor request tuple: ' + case.name)
    return dict(totals=totals, historical_tuples=422, independently_resolved_new_tuples=464)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--capture', type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(check(args.capture), sort_keys=True))


if __name__ == '__main__':
    main()
