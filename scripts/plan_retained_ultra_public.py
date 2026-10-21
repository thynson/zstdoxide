#!/usr/bin/env python3
"""Freeze source-only retained Ultra hypotheses and one exact predecessor transition."""
from pathlib import Path
from dataclasses import replace
import argparse,collections,csv,hashlib,json,sys
ROOT=Path(__file__).resolve().parents[1]
PINS = {'scripts/plan_retained_optimal_public_v3.py': '98d884763ad38f872865054d1c39804b80e4da0e3e6bb0fbf4cf8423e7376789', 'tests/fixtures/reference-retained-optimal-public/recipes.json': '0fcd36c87dd5e1a2de6288c0b3e41abb2acd81b74060f37436baa07d46e57fde', 'scripts/oracles/ultra_retained_recipes.py': 'f0bed380e71c8864ccdacfdd14087cffcbce2386cf621975e4ee2ab96b272e1d', 'scripts/oracles/optimal_retained_wide_recipes.py': '1bcc4d2f5e3bc5b840e5c4639ecea939678f03d9315764967a75bcbc2fec37c6', 'scripts/check_lazy_interop_capture_v2.py': '0cb1ea0cef617b732c02537dfda09f5d4ac3080d2f3e2dccab6cb2c399d8410f', 'scripts/oracles/double_fast_interop.py': '05eefcf492ce7dca0741d3348d165aca8a48e402aa63710d1c043775125705f3', 'tests/upstream/parameters/presets.tsv': '4e188cc16826c6665dcd440ae9d153484aeb4e561321a089be2aec260667881d'}
def need(value,message):
 if not value:raise ValueError(message)
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def classify(values,size):
 from plan_retained_optimal_public_v3 import classify as previous
 old=previous(values,size)
 if values[6]!=8 or not 16385<=size<=32768:return old
 w,c,h,s,_,t,_=values
 return 'success'if size<=1<<w and w<=16 and c<=12 and h<=12 and s<=6 and t<=4095 else 'resource'
def prepare(root=ROOT):
 for name,digest in PINS.items():need(sha(root/name)==digest,'pinned source-only input '+name)
 sys.path.insert(0,str(root/'scripts'))
 from plan_retained_optimal_public_v3 import prepare as preceding
 from oracles.double_fast_interop import Recipe
 from oracles.ultra_retained_recipes import recipes
 from oracles.optimal_retained_wide_recipes import noise
 from check_lazy_interop_capture_v2 import presets,resolved_new_request
 old,prior=preceding(root);stored=json.loads((root/'tests/fixtures/reference-retained-optimal-public/recipes.json').read_bytes())
 need(prior==stored and len(old)==1513,'exact complete retained Optimal predecessor')
 table=presets();cases=[];tuples=[];transitions=[]
 for index,(case,values)in enumerate(zip(old,prior['resolved_tuples'],strict=True)):
  actual=resolved_new_request(case,table);need(actual==values,'independent predecessor resolved tuple')
  expected=classify(values,len(case.source))
  if expected!=case.expected:
   need(case.name=='retained-resource-ultra'and values==[15,12,12,6,3,4095,8]and len(case.source)==32768 and case.expected=='resource'and expected=='success','only predeclared predecessor change')
   transitions.append(dict(index=index,name=case.name,before=case.expected,after=expected,source_sha256=hashlib.sha256(case.source).hexdigest(),request=list(case.request),resolved=values))
  cases.append(replace(case,expected=expected));tuples.append(values)
 need(len(transitions)==1,'one intended transition')
 def add(name,source,request,reason):
  c=Recipe(name,source,request,'success',reason);v=resolved_new_request(c,table)
  cases.append(replace(c,expected=classify(v,len(source))));tuples.append(v)
 for name,recipe in recipes().items():
  tagged=recipe.request;p=tagged.original
  if tagged.policy!=2 or p.literal_mode or len(p.source)<=16384:continue
  add('retained-ultra-'+name,p.source,(13,p.window_log,p.chain_log,p.hash_log,p.search_log,p.min_match,p.target,8,0),'Independent retained source; public balanced schedule and automatic literals.')
 for size in [16385,24576,24577,32768]:
  for kind,source in [('uniform',b'a'*size),('periodic',(b'abcXabcYabcZabcd'*((size+14)//15))[:size]),('noise',noise(size))]:
   add(f'retained-ultra-boundary-{size}-{kind}',source,(13,16,12,12,6,3,4095,8,0),'Balanced block-count and source boundary.')
  for level in [13,14]:add(f'retained-ultra-default-{level}-{size}',b'a'*size,(level,0,0,0,0,0,0,0,0),'Independent default resolution; no forced strategy or enlarged resource caps.')
 for size in [16385,32768]:
  source=(b'abcXabcYabcZabcd'*((size+14)//15))[:size]
  for minimum in range(3,8):
   for target in [0,1,4095]:add(f'retained-ultra-minimum-{minimum}-target-{target}-{size}',source,(13,16,6,6,1,minimum,target,8,0),'Short keys, sufficient-length minimum and path horizon targets.')
 for window in [15,16]:
  for table in [6,12]:
   for search in [1,6]:add(f'retained-ultra-geometry-{window}-{table}-{search}',noise(8192)*4,(13,window,table,table,search,3,128,8,0),'Geometry endpoints and cross-block history.')
 base=(13,16,12,12,6,3,4095,8,0)
 for size in [0,63,32769,65536]:add(f'retained-ultra-resource-source-{size}',b'a'*size,base,'Outside-envelope source rejection.')
 for size in [16385,32768]:
  r=list(base);r[1]=14;add(f'retained-ultra-resource-window-{size}',b'a'*size,tuple(r),'Source beyond resolved window remains a resource error.')
 for label,index,value in [('chain',2,13),('hash',3,13),('search',4,7),('target',6,4096),('ultra2',7,9)]:
  r=list(base);r[index]=value;add('retained-ultra-resource-'+label,b'a'*32768,tuple(r),'Geometry or successor strategy remains outside admission.')
 need(len({c.name for c in cases})==len(cases),'unique case names')
 need(all(0<=len(c.source)<=4<<20 for c in cases)and sum(len(c.source)for c in cases)<=256<<20,'bounded source inventory')
 inventories={}
 for path in sorted((root/'tests/fixtures/reference-admission-v8-v2').glob('*.tsv')):
  count=0
  for row in csv.DictReader(path.open(),delimiter='\t'):
   if row.get('v8')=='advanced':continue
   value=row.get('v8_independent_tuple')or row.get('v7_independent_tuple')or row.get('v6_independent_tuple')or row.get('resolved_tuple',row.get('historical_resolved_tuple'))
   if not value:continue
   values=list(map(int,value.split(',')));size=int(row.get('source_size',row.get('prefix_size')))
   need(classify(values,size)==row['v8'],'unchanged upstream admission inventory');count+=1
  inventories[path.name]=dict(rows=count,sha256=sha(path))
 manifest=dict(schema_version=1,input_sha256=PINS,upstream_revision=prior['upstream_revision'],predecessor_manifest_sha256=sha(root/'tests/fixtures/reference-retained-optimal-public/recipes.json'),predecessor_cases=1513,transitions=transitions,case_count=len(cases),source_bytes=sum(len(c.source)for c in cases),classifications=dict(collections.Counter(c.expected for c in cases)),recipes=[c.metadata()for c in cases],resolved_tuples=tuples,upstream_admission=inventories,qualification='Source-only hypotheses. One predeclared retained-resource-ultra response becomes success; every other predecessor source/request/tuple/classification is identical. New retained strategy8 source16385..32768 must fit its resolved window and existing geometry caps. No native execution or public admission is claimed.')
 return cases,manifest
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--root',type=Path,default=ROOT);p.add_argument('--write',type=Path);args=p.parse_args()
 def guard(event,args):
  if event in ('subprocess.Popen','os.system','os.fork','os.posix_spawn'):raise ValueError('source-only child execution forbidden')
 sys.addaudithook(guard);cases,manifest=prepare(args.root);data=(json.dumps(manifest,sort_keys=True,indent=2)+'\n').encode()
 if args.write:
  args.write.parent.mkdir(parents=True,exist_ok=True)
  with args.write.open('xb')as f:f.write(data)
 print(json.dumps(dict(status='passed',cases=len(cases),transitions=manifest['transitions'],classifications=manifest['classifications'],manifest_sha256=hashlib.sha256(data).hexdigest(),child_processes=0),sort_keys=True))
