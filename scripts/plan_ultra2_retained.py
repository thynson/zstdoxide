#!/usr/bin/env python3
"""Freeze complete independent phase/block predictions before native execution."""
import argparse
import hashlib
import json
from pathlib import Path
import sys
sys.dont_write_bytecode = True
from oracles.ultra2_retained import REVISION, MAX_LINE, MAX_OUTPUT, parse_packet, need
from oracles.ultra2_retained_recipes import recipes
from oracles.ultra2_retained_validation_v4 import records
from oracles.ultra2_protocol import canonical

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT/'tests/fixtures/reference-ultra2-retained-recipes/recipes.json'
PREDECESSOR = ROOT/'tests/fixtures/reference-ultra-retained-recipes/recipes.json'
REQUIRED = {'encoding:ultra_alternative_selected', 'encoding:ultra_alternative_rejected',
            'encoding:ultra_alternative_extends_frontier', 'encoding:later_horizon',
            'encoding:search_short', 'encoding:block_boundary_query',
            'seeding:ultra_alternative_considered', 'compressed_commit', 'raw_rollback',
            'rle_rollback', 'discarded_candidate_repeats'}


def sha(data):return hashlib.sha256(data).hexdigest()


def manifest():
    old_bytes = PREDECESSOR.read_bytes()
    old = {row['name']: row for row in json.loads(old_bytes)['cases']}
    result, coverage = [], set()
    preserved = later_alternatives = 0
    for name, recipe in recipes().items():
        request, kinds = recipe.request, recipe.kinds
        r = request.original
        packet = request.packet()
        need(parse_packet(packet) == request, 'source-only packet roundtrip')
        rows, hit = records(request, kinds)
        coverage.update(hit)
        events = [row for row in rows if row['record'] == 'event']
        alternatives = [row for row in events if row['alternative'] is not None]
        later_alternatives += sum(row['block'] > 0 for row in alternatives)
        sizes = [len(canonical(row).encode()) + 1 for row in rows]
        bound = 2 * (sum(sizes) + 128 * len(rows)) + 4096
        need(bound <= MAX_OUTPUT and max(sizes) + 128 <= MAX_LINE, 'bounded native observation before execution')
        if recipe.predecessor:
            prior = old[recipe.predecessor]
            need(prior['source_sha256'] == sha(r.source) and prior['blocks'] == list(r.blocks)
                 and prior['hypothesized_kinds'] == list(kinds), 'unchanged predecessor input and hypothesis')
            preserved += 1
        resets = [row for row in rows if row['record'] == 'reset']
        gates = [row for row in rows if row['record'] == 'eligibility']
        need([row['seed'] for row in gates] == [r.blocks[0] > 8] + [False] * (len(r.blocks)-1),
             'only eligible first block seeds')
        if resets:
            need(len(resets) == 1 and resets[0]['origin_after'] == r.blocks[0]+2,
                 'first block length determines bias')
        result.append(dict(name=name, predecessor=recipe.predecessor, source_bytes=len(r.source),
            source_sha256=sha(r.source), blocks=list(r.blocks), parameters=[r.window_log, r.hash_log,
                r.chain_log, r.search_log, r.min_match, r.target, r.literal_mode],
            packet_bytes=len(packet), packet_sha256=sha(packet), hypothesized_kinds=list(kinds),
            seeds=len(resets), phases=sum(row['record']=='pass_end' for row in rows),
            events=len(events), records=len(rows), logical_row_bytes=sum(sizes),
            observation_output_bound=bound, maximum_line_bound=max(sizes)+128,
            alternatives_by_block=[sum(row['block']==i for row in alternatives) for i in range(len(r.blocks))],
            highest_path=max((at for row in events for at,_ in row['cells']['changes']),default=0),
            predicted_state_sha256=sha(canonical(rows).encode()), coverage=sorted(hit)))
        print('Predicted ' + name, file=sys.stderr, flush=True)
    need(preserved == 21 and len(result) == 32 and REQUIRED <= coverage and later_alternatives > 0,
         'complete retained witnesses and predecessor inputs')
    return dict(schema_version=1, revision=REVISION, protocol='ZU2RT01', source_cap=32768,
                model='scripts/oracles/ultra2_retained_validation_v4.py',
                predecessor_manifest_sha256=sha(old_bytes), predecessor_inputs=preserved,
                later_alternatives=later_alternatives, coverage=sorted(coverage), cases=result,
                qualification='Thirty-two source-only retained Ultra2 requests include all 21 previous policy-2 inputs, first/later 7/8/9 thresholds, non-power-of-two rebases and future-source variants. Publication kinds are explicit hypotheses, not entropy-selection predictions. No successor native execution, Rust implementation or public admission.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true')
    args = parser.parse_args()
    children = []
    def guard(event, arguments):
        if event in ('subprocess.Popen', 'os.system', 'os.fork', 'os.posix_spawn', 'os.exec', 'os.spawn'):
            children.append(event)
            raise ValueError('native/child execution forbidden')
    sys.addaudithook(guard)
    result = manifest()
    data = (json.dumps(result, sort_keys=True, indent=2)+'\n').encode()
    if args.write:
        need(not DEST.exists(), 'fresh immutable recipe manifest')
        DEST.parent.mkdir(parents=True, exist_ok=True)
        DEST.write_bytes(data)
    need(DEST.read_bytes() == data and not children, 'exact independently regenerated manifest')
    print(json.dumps(dict(status='passed', optimized=not __debug__, child_processes=0,
                         cases=len(result['cases']), predicted_events=sum(row['events'] for row in result['cases']),
                         manifest_sha256=sha(data)), sort_keys=True))


if __name__ == '__main__':main()
