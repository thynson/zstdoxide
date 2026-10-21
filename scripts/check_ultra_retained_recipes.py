#!/usr/bin/env python3
"""Check retained source recipes, later-block alternatives and future-source isolation."""
from pathlib import Path
import copy
import dataclasses
import hashlib
import json
import sys
sys.dont_write_bytecode = True

from oracles.ultra_retained import Request, require, parse_packet
from oracles.ultra_retained_recipes import recipes
from oracles.ultra_retained_validation import records, observations, validate_output
from oracles.optimal_retained_frames_recipes import recipes as predecessors
from oracles.tree_validation import same
from plan_ultra_retained import DEST, REQUIRED

ROOT = Path(__file__).resolve().parents[1]


def encoded(rows):
    return b''.join((json.dumps(r, sort_keys=True, separators=(',', ':'))+'\n').encode()
                    for r in rows)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def check():
    children = []
    def guard(event, args):
        if event in ('subprocess.Popen', 'os.system', 'os.posix_spawn', 'os.fork'):
            children.append(event)
            raise ValueError('child execution forbidden')
    sys.addaudithook(guard)
    manifest_bytes = DEST.read_bytes()
    manifest = json.loads(manifest_bytes)
    cases = recipes()
    prior = predecessors()
    require(len(manifest['cases']) == len(cases) == 42 and manifest['unchanged_policy_zero_cases'] == 19,
            'complete frozen retained request inventory')
    preserved = 0
    for frozen, (name, recipe) in zip(manifest['cases'], cases.items(), strict=True):
        request = recipe.request
        r = request.original
        packet = request.packet()
        require(parse_packet(packet) == request, 'exact recipe transport')
        same({k:frozen[k] for k in ('name', 'policy', 'source_bytes', 'source_sha256',
                                   'packet_bytes', 'packet_sha256', 'nested_packet_sha256',
                                   'parameters', 'blocks', 'hypothesized_kinds')},
             dict(name=name, policy=request.policy, source_bytes=len(r.source), source_sha256=sha(r.source),
                  packet_bytes=len(packet), packet_sha256=sha(packet), nested_packet_sha256=sha(packet[12:]),
                  parameters=[r.window_log, r.hash_log, r.chain_log, r.search_log,
                              r.min_match, r.target, r.literal_mode], blocks=list(r.blocks),
                  hypothesized_kinds=list(recipe.kinds)), 'exact frozen request data')
        old_name = name.removeprefix(f'ur_p{request.policy}_')
        if old_name in prior:
            require(packet[12:] == prior[old_name].request.packet(), 'byte-identical nested predecessor')
            preserved += 1
    require(preserved == 38 and REQUIRED <= set(manifest['coverage']), 'old requests and declared branches')

    branch_rows = []
    for name in ('ur_p2_alternatives_0202', 'ur_p2_alternatives_2020'):
        recipe = cases[name]
        rows, coverage = records(recipe.request, recipe.kinds)
        frozen = next(row for row in manifest['cases'] if row['name'] == name)
        require(sha(json.dumps(rows, sort_keys=True, separators=(',', ':')).encode())
                == frozen['predicted_state_sha256'], 'complete frozen alternative state')
        later = [row for row in rows if row.get('alternative') is not None and row['block'] > 0]
        require(later and sum(frozen['alternatives_by_block'][1:]) == len(later), 'later-block alternatives')
        branch_rows.append(dict(name=name, later_alternatives=len(later), coverage=sorted(coverage)))

    recipe = cases['ur_p2_alternatives_0202']
    request, kinds = recipe.request, recipe.kinds
    good_rows = list(observations(request, kinds))
    good = encoded(good_rows)
    validate_output(good, request, kinds)
    at = next(i for i, row in enumerate(good_rows)
              if row.get('alternative') is not None and row['block'] == 1)
    require(good_rows[at]['alternative'][7] < 0, 'signed LL increment on retained alternative')
    rejected = []
    def forged(name, edit):
        changed = copy.deepcopy(good_rows)
        edit(changed)
        data = encoded(changed)
        require(data != good, 'effective trace mutation: '+name)
        try:
            validate_output(data, request, kinds)
        except ValueError:
            rejected.append(name)
        else:
            raise ValueError('accepted retained alternative forgery: '+name)
    for column in range(12):
        forged('alternative-'+str(column), lambda rows, c=column:
               rows[at]['alternative'].__setitem__(c, rows[at]['alternative'][c]+1))
    forged('following-frontier', lambda rows: rows[at+1].update(last=0))
    forged('following-path', lambda rows: rows[at+1]['cells']['changes'].append([4098, [1,0,0,0,0,0,0]]))
    forged('nonquery-search', lambda rows: rows[at]['search']['changes'].append([0,123]))
    forged('boolean-policy', lambda rows: rows[at].update(price_policy=True))
    forged('wrong-block', lambda rows: rows[at].update(block=0))
    require(len(rejected) == 17, 'all retained alternative forgeries')

    isolated = []
    for policy in (0, 2):
        base = cases[f'ur_p{policy}_alternatives_0202']
        old = base.request.original
        end = old.blocks[0]
        mutated = old.source[:end]+bytes(byte ^ 0x80 for byte in old.source[end:])
        require(mutated != old.source, 'actual future-source mutation')
        changed = Request(policy, dataclasses.replace(old, source=mutated))
        before, _ = records(base.request, base.kinds)
        after, _ = records(changed, base.kinds)
        first_before = [row for row in before if row['block'] == 0]
        first_after = [row for row in after if row['block'] == 0]
        same(first_after, first_before, 'all first-block state ignores future backing bytes')
        require(encoded(after) != encoded(before), 'changed suffix is exercised later')
        isolated.append(dict(policy=policy, source_sha256=sha(old.source),
                             mutated_source_sha256=sha(mutated), first_block_records=len(first_before),
                             first_block_sha256=sha(encoded(first_before))))
    require(not children, 'no child execution')
    source = {str(Path(m.__file__).resolve().relative_to(ROOT)):
              sha(Path(m.__file__).resolve().read_bytes()) for m in list(sys.modules.values())
              if getattr(m, '__file__', None) and Path(m.__file__).suffix == '.py'
              and Path(m.__file__).resolve().is_relative_to(ROOT/'scripts')}
    return dict(status='passed', optimized=not __debug__, cases=42, preserved_nested_requests=38,
                manifest_sha256=sha(manifest_bytes), branch_cases=branch_rows,
                rejected=rejected, future_source_isolation=isolated, source_sha256=source,
                child_processes=0, qualification='Source-only complete request binding, later-block alternative controls and consumed-prefix isolation. The manifest planner separately replays all42complete states. Publication kinds remain hypotheses; no successor native execution or production retained Ultra admission.')


if __name__ == '__main__':
    print(json.dumps(check(), sort_keys=True, indent=2))
