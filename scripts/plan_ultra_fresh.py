#!/usr/bin/env python3
"""Freeze complete source-only ordinary coupled-policy state expectations."""
import argparse
import hashlib
import json
from pathlib import Path
from oracles.ultra_fresh import REVISION, require
from oracles.ultra_fresh_recipes import recipes
from oracles.ultra_fresh_validation import records

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / 'tests/fixtures/reference-ultra-fresh-recipes/capture-plan.json'


def manifest():
    cases = []
    for name, recipe in recipes().items():
        rows, coverage = records(recipe.request, recipe.kinds)
        digest = hashlib.sha256()
        size = 0
        for row in rows:
            data = (json.dumps(row, sort_keys=True, separators=(',', ':')) + '\n').encode()
            digest.update(data)
            size += len(data)
        events = [r for r in rows if r['record'] == 'event']
        cases.append(dict(name=name, policy=recipe.request.policy,
            packet_sha256=hashlib.sha256(recipe.request.packet()).hexdigest(),
            rows_sha256=digest.hexdigest(), logical_row_bytes=size,
            hypothesized_kinds=list(recipe.kinds), events=len(events),
            alternatives=sum(e['alternative'] is not None for e in events),
            queries=sum(e['stage'] == 'query' for e in events),
            sequences=rows[-1]['sequences'], coverage=sorted(coverage)))
    require(len(cases) == 44 and sum(c['events'] for c in cases) == 198476
            and sum(c['alternatives'] for c in cases) == 201, 'original source-only totals')
    return dict(schema_version=1, revision=REVISION, cases=cases,
        qualification='Source-only ordinary search/path expectations; publication kinds are explicit hypotheses, not entropy-selection predictions.')


def main(write):
    data = (json.dumps(manifest(), sort_keys=True, indent=2) + '\n').encode()
    if write:
        require(not DEST.exists(), 'fresh prediction manifest')
        DEST.write_bytes(data)
    require(DEST.read_bytes() == data, 'exact source-only predictions')
    print(json.dumps(dict(status='passed', cases=44, sha256=hashlib.sha256(data).hexdigest())))


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--write', action='store_true')
    main(p.parse_args().write)
