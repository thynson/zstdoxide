#!/usr/bin/env python3
"""Preflight the frozen policy-tagged frequency inventory without native runs."""
from pathlib import Path
import copy,json,shutil,sys
sys.dont_write_bytecode=True
import ultra_frequency_capture_plan as p
from oracles import ultra_scalar as m
upstream=Path(sys.argv[1]).resolve();output=Path(sys.argv[2]).resolve();compiler=Path(shutil.which('clang')).resolve()
source=p.source_manifest();_,units=p.reference_sources(upstream);plan=p.plan(upstream,output,compiler,units)
m.need(p.validate_plan(plan,upstream,output,compiler,units)==702,'exact command count')
m.need(len(p.requests())==320 and len(p.malformed_packets())==20,'complete request inventory')
outcomes=p.outcomes();m.need({n for n,(exit,_)in outcomes.items()if exit and not n.startswith('negative-')}=={'uf_p2_literals_131071_x32','uf_p2_fraction_boundary_31'},'unchanged original and new boundary negatives')
m.need(sum(r['expected_exit']==0 and '-run-uf_'in r['label']for r in plan)==596,'both complete positive schedules')
changes=[]
for label in ['normal-compile-0','sanitized-compile-0','sanitized-compile-25','normal-run-uf_p2_init_single_9_m0']:
 def mutate(rows,label=label):next(r for r in rows if r['label']==label)['argv'].append('-DUNREVIEWED=1')
 changes.append(mutate)
changes += [lambda rows:rows.pop(),lambda rows:rows.reverse(),lambda rows:rows[0].update(expected_exit=True),lambda rows:rows[0].update(timeout_seconds=121),lambda rows:rows[0].update(expected_diagnostic='foreign'),lambda rows:rows.append(copy.deepcopy(rows[0]))]
for change in changes:
 changed=copy.deepcopy(plan);change(changed)
 try:p.validate_plan(changed,upstream,output,compiler,units)
 except ValueError:pass
 else:raise ValueError('changed inventory accepted')
m.need(source==p.source_manifest(),'unchanged preflight source')
print(json.dumps(dict(status='passed',optimized=not __debug__,commands=len(plan),requests=320,malformed=20,rejected_plans=len(changes),source=source),sort_keys=True))
