#!/usr/bin/env python3
"""Source-only boundary and predecessor controls for retained Ultra admission."""
from pathlib import Path
import argparse,hashlib,json,sys
from plan_retained_ultra_public import classify,prepare,need

def check(root):
 cases,manifest=prepare(root)
 base=[15,12,12,6,3,4095,8];rejected=0;positive=0
 def reject(call):
  nonlocal rejected
  try:call()
  except(ValueError,TypeError):rejected+=1
  else:raise ValueError('malformed policy input accepted')
 for size in [True,-1,1.0,(1<<64)-1,None]:reject(lambda n=size:classify(base,n))
 for values in [None,(),[],[15,12,12,6,3,4095,True],[15,12,12,6.0,3,4095,8],[9,12,12,6,3,4095,8],[15,5,12,6,3,4095,8],[15,12,12,0,3,4095,8],[15,12,12,6,2,4095,8],[15,12,12,6,3,131073,8]]:reject(lambda v=values:classify(v,16385))
 for size,want in [(0,'resource'),(63,'resource'),(64,'success'),(16384,'success'),(16385,'success'),(32768,'success'),(32769,'resource')]:need(classify(base,size)==want,'source boundary');positive+=1
 for at,value in [(0,17),(1,13),(2,13),(3,7),(5,4096)]:
  bad=base.copy();bad[at]=value;need(classify(bad,32768)=='resource','geometry cap');positive+=1
 for size in [16385,32768]:
  for strategy in [8,9]:
   bad=base.copy();bad[0]=14;bad[6]=strategy;need(classify(bad,size)=='resource','window remains closed');positive+=1
  bad=base.copy();bad[6]=9;need(classify(bad,size)=='resource','retained Ultra2 stays closed');positive+=1
 for minimum in range(3,8):
  for target in [0,1,4095]:
   good=base.copy();good[4]=minimum;good[5]=target;need(classify(good,32768)=='success','minimum and target modes');positive+=1
 transition=manifest['transitions'][0];old_count=manifest['predecessor_cases']
 need(old_count==1513 and transition['name']=='retained-resource-ultra'and transition['before']=='resource'and transition['after']=='success','one exact declared transition')
 need(sum(c.name=='retained-resource-ultra'for c in cases)==1,'unique transition identity')
 need(rejected==15 and positive==33,'exact boundary control inventory')
 data=(json.dumps(manifest,sort_keys=True,indent=2)+'\n').encode()
 return dict(status='passed',cases=len(cases),classifications=manifest['classifications'],predecessors=1513,transitions=1,negative_controls=rejected,positive_controls=positive,manifest_sha256=hashlib.sha256(data).hexdigest(),child_processes=0)
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--root',type=Path,default=Path(__file__).resolve().parents[1]);a=p.parse_args()
 def guard(event,args):
  if event in ('subprocess.Popen','os.system','os.fork','os.posix_spawn'):raise ValueError('source-only child execution forbidden')
 sys.addaudithook(guard);print(json.dumps(check(a.root),sort_keys=True))
