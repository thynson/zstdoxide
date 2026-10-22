#!/usr/bin/env python3
"""Source-only malformed, boundary and predecessor controls for retained Ultra2."""
from pathlib import Path
import argparse,hashlib,json,sys
from plan_retained_ultra2_public import classify,prepare,need

def check(root):
 cases,manifest=prepare(root)
 from plan_retained_ultra_public_v3 import classify as previous
 base=[15,12,12,6,3,4095,9];rejected=0;positive=0
 def reject(call):
  nonlocal rejected
  try:call()
  except(ValueError,TypeError):rejected+=1
  else:raise ValueError('malformed policy input accepted')
 for size in [True,-1,1.0,(1<<64)-1,None]:reject(lambda n=size:classify(base,n))
 for values in [None,(),[],[15,12,12,6,3,4095,True],[15,12,12,6.0,3,4095,9],[9,12,12,6,3,4095,9],[15,5,12,6,3,4095,9],[15,12,12,0,3,4095,9],[15,12,12,6,2,4095,9],[15,12,12,6,3,131073,9]]:reject(lambda v=values:classify(v,16385))
 for size,want in [(0,'resource'),(63,'resource'),(64,'success'),(16384,'success'),(16385,'success'),(32768,'success'),(32769,'resource')]:need(classify(base,size)==want,'source boundary');positive+=1
 for at,value in [(0,17),(1,13),(2,13),(3,7),(5,4096)]:
  bad=base.copy();bad[at]=value;need(classify(bad,32768)=='resource','geometry cap');positive+=1
 for size in [16385,32768]:
  bad=base.copy();bad[0]=14;need(classify(bad,size)=='resource','window remains closed');positive+=1
 for strategy in [6,7,8]:
  values=base.copy();values[6]=strategy;need(classify(values,32768)==previous(values,32768),'sibling strategy unchanged');positive+=1
 for minimum in range(3,8):
  for target in [0,1,4095]:
   good=base.copy();good[4]=minimum;good[5]=target;need(classify(good,32768)=='success','minimum and target modes');positive+=1
 transitions=manifest['transitions']
 need(manifest['predecessor_cases']==1593 and [(r['index'],r['name'],r['before'],r['after'])for r in transitions]==[(1512,'retained-resource-ultra2','resource','success'),(1592,'retained-ultra-resource-ultra2','resource','success')],'two exact declared transitions')
 need(all(sum(c.name==r['name']for c in cases)==1 for r in transitions),'unique transition identities')
 need(rejected==15 and positive==32,'exact boundary control inventory')
 data=(json.dumps(manifest,sort_keys=True,indent=2)+'\n').encode()
 return dict(status='passed',cases=len(cases),classifications=manifest['classifications'],predecessors=1593,transitions=2,negative_controls=rejected,positive_controls=positive,manifest_sha256=hashlib.sha256(data).hexdigest(),child_processes=0)
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--root',type=Path,required=True);a=p.parse_args()
 def guard(event,args):
  if event in ('subprocess.Popen','os.system','os.fork','os.posix_spawn'):raise ValueError('source-only child execution forbidden')
 sys.addaudithook(guard);print(json.dumps(check(a.root),sort_keys=True))
