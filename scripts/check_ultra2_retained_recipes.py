#!/usr/bin/env python3
"""Check source-only retained Ultra2 witnesses and coherent later-phase forgeries."""
import copy
import hashlib
import json
from pathlib import Path
import sys
sys.dont_write_bytecode = True
from oracles.ultra2_retained import parse_packet, need
from oracles.ultra2_retained_recipes import recipes
from oracles.ultra2_retained_validation_v4 import records, observations, validate_output
from oracles.ultra2_protocol import canonical
from plan_ultra2_retained import DEST, REQUIRED


def sha(data):return hashlib.sha256(data).hexdigest()

def encode(rows):return b''.join((canonical(row)+'\n').encode() for row in rows)


def check():
    children = []
    def guard(event, arguments):
        if event in ('subprocess.Popen','os.system','os.fork','os.posix_spawn','os.exec','os.spawn'):
            children.append(event)
            raise ValueError('source-only recipe checks forbid children')
    sys.addaudithook(guard)
    data = DEST.read_bytes()
    manifest = json.loads(data)
    cases = recipes()
    need(len(cases)==len(manifest['cases'])==32 and manifest['predecessor_inputs']==21,
         'complete frozen source inventory')
    need(REQUIRED <= set(manifest['coverage']), 'frozen required branch witnesses')
    for frozen,(name,recipe) in zip(manifest['cases'],cases.items(),strict=True):
        r = recipe.request.original
        packet = recipe.request.packet()
        need(parse_packet(packet)==recipe.request and frozen['name']==name and
             frozen['packet_sha256']==sha(packet) and frozen['source_sha256']==sha(r.source)
             and frozen['blocks']==list(r.blocks) and frozen['hypothesized_kinds']==list(recipe.kinds),
             'exact source, schedule, packet and publication hypothesis')
    base = cases['u2r_alternatives_0202']
    base_rows, _ = records(base.request,base.kinds)
    frozen = next(row for row in manifest['cases'] if row['name']=='u2r_alternatives_0202')
    need(sha(canonical(base_rows).encode())==frozen['predicted_state_sha256'], 'complete frozen witness state')
    first = [row for row in base_rows if row['block']==0]
    isolation = []
    for mask in (128,255):
        case = cases[f'u2r_future_{mask}']
        changed, _ = records(case.request,case.kinds)
        need(canonical([row for row in changed if row['block']==0])==canonical(first),
             'all discarded and actual first-block states ignore future bytes')
        need(canonical(changed)!=canonical(base_rows), 'mutated future bytes are exercised later')
        isolation.append(dict(mask=mask,first_records=len(first),first_sha256=sha(encode(first)),
                              changed_source_sha256=sha(case.request.original.source)))
    good = list(observations(base.request,base.kinds))
    raw = encode(good)
    validate_output(raw,base.request,base.kinds)
    later = next(i for i,row in enumerate(good) if row.get('alternative') is not None and row['block']>0)
    rejected = []
    for column in range(12):
        rows = copy.deepcopy(good)
        rows[later]['alternative'][column] += 1
        changed = encode(rows)
        need(changed!=raw, 'effective later alternative forgery')
        try:validate_output(changed,base.request,base.kinds)
        except ValueError:rejected.append('later-alternative-'+str(column))
        else:raise ValueError('accepted later alternative forgery')
    for field in ('block','padding_pass','pass_name','stage'):
        rows = copy.deepcopy(good)
        rows[later][field] = {'block':0,'padding_pass':1,'pass_name':'seeding','stage':'query'}[field]
        changed=encode(rows);need(changed!=raw,'effective later phase forgery')
        try:validate_output(changed,base.request,base.kinds)
        except ValueError:rejected.append('later-'+field)
        else:raise ValueError('accepted later phase forgery')
    thresholds=[]
    for location in ('first','later'):
        for length in (7,8,9):
            name=f'u2r_{location}_threshold_{length}'
            recipe=cases[name];rows,_=records(recipe.request,recipe.kinds)
            gates=[row for row in rows if row['record']=='eligibility']
            need([row['seed'] for row in gates]==[location=='later' or length>8,False],
                 'independent parser-entry and bootstrap thresholds')
            thresholds.append(dict(name=name,seed=[row['seed'] for row in gates]))
    need(not children and len(rejected)==16,'complete pure controls')
    return dict(status='passed',optimized=not __debug__,child_processes=0,manifest_sha256=sha(data),
                cases=32,later_alternative_forgeries=rejected,future_isolation=isolation,thresholds=thresholds,
                qualification='Source-only witnesses and coherent later-phase controls; publication hypotheses are not native observations.')


if __name__=='__main__':print(json.dumps(check(),sort_keys=True,indent=2))
