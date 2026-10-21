#!/usr/bin/env python3
"""Freeze retained policy inputs and complete independent state expectations."""
import argparse
import hashlib
import json
from pathlib import Path
import sys
sys.dont_write_bytecode = True

from oracles.ultra_retained import REVISION, require, parse_packet
from oracles.ultra_retained_recipes import recipes
from oracles.ultra_retained_validation import records, MAX_OUTPUT, MAX_LINE
from check_ultra_retained_model import predecessor_projection

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT/'tests/fixtures/reference-ultra-retained-recipes/recipes.json'
PREDECESSOR = ROOT/'tests/fixtures/reference-optimal-retained-frames-recipes/recipes.json'
REQUIRED = {'ultra_alternative_considered', 'ultra_alternative_extends_frontier',
            'ultra_alternative_rejected', 'ultra_alternative_selected',
            'ultra_improvement_after_rejection', 'ultra_match_tie_continue',
            'ultra_match_worse_continue', 'ultra_unpromising_query',
            'later_horizon', 'raw_rollback', 'rle_rollback', 'compressed_commit',
            'block_boundary_query', 'search_short', 'discarded_candidate_repeats'}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()


def manifest():
    old_bytes = PREDECESSOR.read_bytes()
    old = {r['name']: r for r in json.loads(old_bytes)['cases']}
    result = []
    all_coverage = set()
    predecessors = 0
    later_alternatives = 0
    for name, recipe in recipes().items():
        request = recipe.request
        r = request.original
        packet = request.packet()
        require(parse_packet(packet) == request, 'exact immutable recipe packet')
        rows, coverage = records(request, recipe.kinds)
        events = [row for row in rows if row['record'] == 'event']
        alternatives = [row for row in events if row['alternative'] is not None]
        row_bytes = [len(canonical(row))+1 for row in rows]
        output_bound = 2*(sum(row_bytes)+128*len(rows))+4096
        require(output_bound <= MAX_OUTPUT and max(row_bytes, default=0)+128 <= MAX_LINE,
                'source-derived bounded observation extent before native execution')
        if request.policy == 2:
            all_coverage.update(coverage)
            later_alternatives += sum(e['block'] > 0 for e in alternatives)
        original_name = name.removeprefix(f'ur_p{request.policy}_')
        if original_name in old:
            prior = old[original_name]
            require(sha(packet[12:]) == prior['packet_sha256'], 'unchanged nested predecessor request')
            require(list(recipe.kinds) == prior['hypothesized_kinds'], 'unchanged predecessor publication hypotheses')
            if request.policy == 0:
                projected = [predecessor_projection(row) for row in rows]
                require(sha(canonical(projected)) == prior['predicted_state_sha256'],
                        'exact complete predecessor state hypotheses')
                require(sorted(coverage) == prior['coverage'], 'exact predecessor coverage')
                predecessors += 1
        result.append(dict(name=name, policy=request.policy, source_bytes=len(r.source),
                           source_sha256=sha(r.source), packet_bytes=len(packet),
                           packet_sha256=sha(packet), nested_packet_sha256=sha(packet[12:]),
                           parameters=[r.window_log, r.hash_log, r.chain_log, r.search_log,
                                       r.min_match, r.target, r.literal_mode],
                           blocks=list(r.blocks), hypothesized_kinds=list(recipe.kinds),
                           logical_row_bytes=sum(row_bytes), observation_output_bound=output_bound,
                           events=len(events), queries=sum(e['stage'] == 'query' for e in events),
                           sequences=sum(row['sequences'] for row in rows if row['record'] == 'block_publish'),
                           alternatives=len(alternatives),
                           alternatives_by_block=[sum(e['block'] == i for e in alternatives)
                                                  for i in range(len(r.blocks))],
                           highest_path=max((at for e in events for at, _ in e['cells']['changes']), default=0),
                           predicted_state_sha256=sha(canonical(rows)), coverage=sorted(coverage)))
    require(len(result) == 42 and predecessors == 19 and REQUIRED <= all_coverage,
            'complete predecessor and retained Ultra branch inventory')
    require(later_alternatives > 0, 'actual alternative events after first block')
    require(all(sum(c['alternatives_by_block'][1:]) > 0 for c in result
                if c['name'].startswith('ur_p2_alternatives_')), 'both added sources exercise retained alternatives')
    return dict(schema_version=1, revision=REVISION, protocol='ZURTS01', source_cap=32768,
                predecessor_manifest_sha256=sha(old_bytes), unchanged_policy_zero_cases=predecessors,
                policy_two_later_alternatives=later_alternatives, coverage=sorted(all_coverage),
                cases=result, qualification='Forty-two source-only retained requests preserve all nineteen predecessor packets under both policies and every policy-zero state hypothesis. Policy-two alternatives occur after the first block. Publication kinds are explicit hypotheses, not entropy-selection predictions. No successor native execution or public retained Ultra admission.')


def main(write):
    children = []
    def guard(event, args):
        if event in ('subprocess.Popen', 'os.system', 'os.posix_spawn', 'os.fork'):
            children.append(event)
            raise ValueError('child execution forbidden')
    sys.addaudithook(guard)
    result = manifest()
    require(not children, 'no child execution')
    data = (json.dumps(result, sort_keys=True, indent=2)+'\n').encode()
    if write:
        require(not DEST.exists(), 'fresh immutable retained Ultra manifest')
        DEST.parent.mkdir(parents=True, exist_ok=True)
        DEST.write_bytes(data)
    require(DEST.read_bytes() == data, 'exact frozen retained Ultra manifest')
    print(json.dumps(dict(status='passed', optimized=not __debug__, cases=len(result['cases']),
                         manifest_sha256=sha(data), policy_two_later_alternatives=result['policy_two_later_alternatives'],
                         predicted_events=sum(c['events'] for c in result['cases']), child_processes=0), sort_keys=True))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true')
    main(parser.parse_args().write)
