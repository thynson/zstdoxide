#!/usr/bin/env python3
"""Read-only predecessor and new source schedule reconciliation; no native code."""
import hashlib,json,sys
from pathlib import Path
from oracles.optimal_retained_frames_recipes import recipes
from oracles.optimal_retained_wide_recipes import recipes as predecessors
from oracles.optimal_retained_frames import balanced_blocks,require
R=Path(__file__).resolve().parents[1]

def check():
 children=[]
 def guard(event,args):
  if event in ('subprocess.Popen','os.system','os.fork','os.posix_spawn'):
   children.append(event);raise ValueError('no child execution')
 sys.addaudithook(guard)
 old=json.loads((R/'tests/fixtures/reference-optimal-retained-wide-recipes/recipes.json').read_text())
 new=json.loads((R/'tests/fixtures/reference-optimal-retained-frames-recipes/recipes.json').read_text())
 cases=recipes();prior=predecessors()
 require(len(new['cases'])==len(cases)==19 and new['source_cap']==32768,'new hypothesis inventory')
 require(new['revision']==old['revision'],'pinned source revision')
 for a,b in zip(old['cases'],new['cases'][:7],strict=True):
  require({k:v for k,v in a.items()if k!='packet_sha256'}=={k:v for k,v in b.items()if k!='packet_sha256'},'unchanged predecessor states and publication hypotheses')
  name=a['name'];require(cases[name].request.packet()[8:]==prior[name].request.packet()[8:],'unchanged predecessor request except protocol magic')
 for row,(name,recipe)in zip(new['cases'],cases.items(),strict=True):
  require(row['name']==name and row['packet_sha256']==hashlib.sha256(recipe.request.packet()).hexdigest(),'exact frozen source hypothesis')
  require(row['blocks']==list(recipe.request.blocks) and row['hypothesized_kinds']==list(recipe.kinds),'exact schedule/publication hypothesis')
  if name not in prior:require(recipe.request.blocks==balanced_blocks(len(recipe.request.source)),'new source-only balanced schedule')
 require(not children,'no children')
 return dict(status='passed',recipes=19,unchanged_predecessors=7,new_hypotheses=12,predicted_events=sum(r['events']for r in new['cases']),child_processes=0,manifest_sha256=hashlib.sha256((R/'tests/fixtures/reference-optimal-retained-frames-recipes/recipes.json').read_bytes()).hexdigest(),qualification='Source-only hypotheses and predecessor state hashes; no larger native or Rust execution claim.')
if __name__=='__main__':print(json.dumps(check(),sort_keys=True))
