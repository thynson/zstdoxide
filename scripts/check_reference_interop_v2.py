#!/usr/bin/env python3
"""Apply frozen admission v2 to the original bounded bidirectional campaign.

The original driver, protocol, recipes and historical evidence remain immutable.
Only expected admission changes. The existing helper-only ASan/UBSan scope does
not instrument its prebuilt C library. Larger DoubleFast campaigns are separate.
"""
from pathlib import Path
import argparse
import copy
import csv
import io
import json

import check_reference_admission_v2 as policy


def plan():
    # Check identities before importing/reusing the original executable driver.
    policy.check()
    import check_reference_fast_interop as original
    rows = list(csv.DictReader(io.StringIO(
        (policy.DESTINATION / 'interop.tsv').read_text()), delimiter='\t'))
    recipes = original.recipes()
    policy.require(len(rows) == len(recipes) == 92, 'exact successor recipe count')
    for row, recipe in zip(rows, recipes):
        policy.require((row['case'], int(row['source_size']), int(row['level']),
                        int(row['hint']), row['overrides'], row['historical']) ==
                       (recipe['id'], recipe['size'], recipe['level'], recipe['hint'],
                        ','.join(map(str, recipe['overrides'])), recipe['expected']),
                       'unchanged input recipe')
        policy.require(row['source_sha256'] == policy.sha(original.source(recipe)),
                       'unchanged source recipe')
        recipe['expected'] = row['successor']
    return original, rows, recipes


def execute(args):
    original, rows, recipes = plan()
    expected = {r['case']: [int(v) for v in r['historical_resolved_tuple'].split(',')]
                for r in rows}
    metadata = dict(version=2, historical_recipe_count=92,
                    newly_admitted=[r['case'] for r in rows if r['additive_c_required'] == '1'],
                    qualification='Successor admission only; unchanged bounded inputs and '
                    'protocol. Historical resolved tuples are Rust observations, not C scalars.')
    old_recipes, old_owned, old_write = original.recipes, original.OWNED, original.write_json

    def write(path, value):
        if path.name == 'evidence.json':
            value['admission_policy'] = metadata
            # Every completed observation must preserve scalar resolution, even
            # when its status changes. Validate before persisting the evidence.
            for case in value.get('cases', []):
                for result in case['rust'].values():
                    policy.require(result['resolved'] == expected[case['recipe']['id']],
                                   'successor changed historical scalar resolution')
        old_write(path, value)

    original.recipes = lambda: copy.deepcopy(recipes)
    original.OWNED += ('scripts/check_reference_interop_v2.py',
                       'scripts/check_reference_admission_v2.py',
                       *tuple(str(p.relative_to(policy.ROOT))
                              for p in sorted(policy.DESTINATION.iterdir())),
                       *tuple(policy.PINS))
    original.write_json = write
    try:
        original.execute(args)
    finally:
        original.recipes, original.OWNED, original.write_json = old_recipes, old_owned, old_write


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--upstream', type=Path)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--compiler')
    parser.add_argument('--check-plan', action='store_true')
    args = parser.parse_args()
    if args.check_plan:
        policy.require(args.upstream is None and args.output is None and args.compiler is None,
                       '--check-plan excludes execution arguments')
        _, rows, recipes = plan()
        print(json.dumps(dict(version=2, recipes=len(recipes),
                              source_bytes=sum(r['size'] for r in recipes),
                              max_source=max(r['size'] for r in recipes),
                              statuses={s: sum(r['expected'] == s for r in recipes)
                                        for s in ('success', 'unsupported', 'resource')},
                              additive_c_required=[r['case'] for r in rows
                                                   if r['additive_c_required'] == '1']),
                         sort_keys=True))
    else:
        policy.require(args.upstream is not None and args.output is not None,
                       '--upstream and fresh --output are required')
        execute(args)


if __name__ == '__main__':
    main()
