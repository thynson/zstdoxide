#!/usr/bin/env python3
"""Preflight pure native inventory and reject changed commands; no codec run."""
import copy,json,shutil,sys
from pathlib import Path
sys.dont_write_bytecode=True
import ultra_scalar_capture_plan as p
from oracles import ultra_scalar as m

upstream=Path(sys.argv[1]).resolve();output=Path(sys.argv[2]).resolve();compiler=Path(shutil.which('clang')).resolve()
source=p.source_manifest();_,units=p.reference_sources(upstream);plan=p.plan(upstream,output,compiler,units)
m.need(len(units)==25 and len([r for r in plan if '-compile-'in r['label']])==52,'complete codec compilation')
m.need(len(p.malformed_packets())==35,'frozen malformed cases')
m.need(p.validate_plan(plan,upstream,output,compiler,units)==134,'fixed command count')
changes=[]
for label in ['normal-compile-0','sanitized-compile-0','sanitized-compile-25','normal-run-scalar-boundaries']:
 def mutate(rows,label=label):next(r for r in rows if r['label']==label)['argv'].append('-DUNREVIEWED=1')
 changes.append(mutate)
changes += [lambda rows:rows.pop(),lambda rows:rows.reverse(),lambda rows:rows[0].update(expected_exit=True),lambda rows:rows[0].update(timeout_seconds=121),lambda rows:rows[0].update(stdout_limit=1),lambda rows:rows.append(copy.deepcopy(rows[0]))]
for change in changes:
 altered=copy.deepcopy(plan);change(altered)
 try:p.validate_plan(altered,upstream,output,compiler,units)
 except ValueError:pass
 else:raise ValueError('changed plan accepted')
m.need(p.source_manifest()==source,'unchanged preflight source')
print(json.dumps(dict(status='passed',optimized=not __debug__,commands=len(plan),malformed=len(p.malformed_packets()),rejected_plans=len(changes),source=source),sort_keys=True))
