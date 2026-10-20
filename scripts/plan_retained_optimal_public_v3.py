#!/usr/bin/env python3
"""Additive source-only retained Optimal public hypotheses over Ultra2 history."""
import argparse,collections,csv,hashlib,json,sys
from dataclasses import replace
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
PINS={'scripts/oracles/ultra2_interop.py': '8ac8b792373196b449a2d9fa0d4eaf636c65d46dc5c01790d91fc2b5077319c4', 'scripts/oracles/double_fast_interop.py': '05eefcf492ce7dca0741d3348d165aca8a48e402aa63710d1c043775125705f3', 'scripts/oracles/optimal_retained_frames_recipes.py': 'acb0f4c48fc2775c0abeaab7ac46bc5a5444d58c70a91fd911d37b8c97060223', 'scripts/oracles/optimal_retained_frames.py': 'dd60a695a11d6e044d9efd0344376cb081df14e5646709c83c9ef5e978aca482', 'scripts/oracles/optimal_retained_wide_recipes.py': '1bcc4d2f5e3bc5b840e5c4639ecea939678f03d9315764967a75bcbc2fec37c6', 'scripts/check_reference_admission_v8_v2.py': '3146694f9fcb7b0e1a2cfb7c2654df52f7049dd5c9aaf0e9fc0219e0317ffe29', 'scripts/check_reference_admission_v7.py': 'e7d00a7b657bd518cc41d81a5076f6d1f831c511862643e40e92252d4c6df93f', 'scripts/check_reference_admission_v6.py': 'b73e2d9190ea5450c86d23b7e303826da8693a2fcb7c22f6d5a48ac186335391', 'scripts/check_lazy_interop_capture_v2.py': '0cb1ea0cef617b732c02537dfda09f5d4ac3080d2f3e2dccab6cb2c399d8410f', 'tests/upstream/parameters/presets.tsv': '4e188cc16826c6665dcd440ae9d153484aeb4e561321a089be2aec260667881d', 'tests/fixtures/reference-ultra2-interop-recipes/recipes.json': '0ae6c7cfb5ab3530c52976139326514e9919bcba321d9e6516621ffd129f4dd5'}
def need(ok,why):
 if not ok:raise ValueError(why)
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def classify(values,size):
 from check_reference_admission_v8_v2 import classify as old
 previous=old(values,size)
 if values[6]!=7 or not 16385<=size<=32768:return previous
 w,c,h,s,_,t,_=values
 return 'success'if size<=1<<w and w<=16 and c<=12 and h<=12 and s<=6 and t<=4095 else'resource'
def prepare(root=ROOT):
 for name,digest in PINS.items():need(sha(root/name)==digest,'pinned source-only input '+name)
 sys.path.insert(0,str(root/'scripts'))
 from oracles import ultra2_interop as prior
 from oracles.double_fast_interop import Recipe
 from oracles.optimal_retained_frames_recipes import recipes as retained
 from oracles.optimal_retained_wide_recipes import noise
 from check_lazy_interop_capture_v2 import presets,resolved_new_request
 old=prior.recipes();prior_manifest=json.loads((root/'tests/fixtures/reference-ultra2-interop-recipes/recipes.json').read_bytes())
 need([c.metadata()for c in old]==prior_manifest['recipes']and len(old)==1433,'complete unchanged predecessor inventory')
 table=presets();cases=[];tuples=[]
 for c,v in zip(old,prior_manifest['resolved_tuples'],strict=True):
  actual=resolved_new_request(c,table) if v[6]==7 else v
  need(actual==v and classify(actual,len(c.source))==c.expected,'every predecessor request/tuple/classification unchanged')
  cases.append(c);tuples.append(v)
 def add(name,source,request,reason):
  c=Recipe(name,source,request,'success',reason);v=resolved_new_request(c,table)
  c=replace(c,expected=classify(v,len(source)));cases.append(c);tuples.append(v)
 for name,r in retained().items():
  p=r.request
  if len(p.source)<=16384 or p.literal_mode:continue
  add('retained-'+name,p.source,(11,p.window_log,p.chain_log,p.hash_log,p.search_log,p.min_match,p.target,7,0),'Independent larger retained source; public balanced schedule and automatic literals.')
 for size in [16385,24576,24577,32768]:
  for kind,source in [('uniform',b'a'*size),('periodic',(b'abcXabcYabcZabcd'*((size+14)//15))[:size]),('noise',noise(size))]:
   add(f'retained-boundary-{size}-{kind}',source,(11,16,12,12,6,3,4095,7,0),'Additive balanced block-count/source boundary.')
  for level in [11,12]:add(f'retained-default-{level}-{size}',b'a'*size,(level,0,0,0,0,0,0,0,0),'Independent default resolution; resource caps still apply.')
 for size in [16385,32768]:
  source=(b'abcXabcYabcZabcd'*((size+14)//15))[:size]
  for minimum in range(3,8):
   for target in [0,1,4095]:add(f'retained-minimum-{minimum}-target-{target}-{size}',source,(11,16,6,6,1,minimum,target,7,0),'Short keys, sufficient-length minimum and horizon targets.')
 for window in [15,16]:
  for table_log in [6,12]:
   for search in [1,6]:add(f'retained-geometry-{window}-{table_log}-{search}',noise(8192)*4,(11,window,table_log,table_log,search,3,1,7,0),'Independent geometry endpoints and cross-block history.')
 base=(11,16,12,12,6,3,4095,7,0)
 for size in [0,63,32769,65536]:add(f'retained-resource-source-{size}',b'a'*size,base,'Preserved outside-envelope source rejection.')
 for label,index,value in [('window',1,14),('chain',2,13),('hash',3,13),('search',4,7),('target',6,4096),('ultra',7,8),('ultra2',7,9)]:
  r=list(base);r[index]=value;add('retained-resource-'+label,b'a'*32768,tuple(r),'Retained geometry or successor strategy remains outside admission.')
 need(len({c.name for c in cases})==len(cases),'unique names')
 need(all(0<=len(c.source)<=4<<20 for c in cases)and sum(len(c.source)for c in cases)<=256<<20,'bounded aggregate sources')
 # Reclassify all prior upstream-admission rows, including sources absent from
 # the public interop corpus; none may silently change a frozen expectation.
 inventories={}
 for path in sorted((root/'tests/fixtures/reference-admission-v8-v2').glob('*.tsv')):
  count=0
  for row in csv.DictReader(path.open(),delimiter='\t'):
   if row.get('v8')=='advanced':continue
   value=row.get('v8_independent_tuple')or row.get('v7_independent_tuple')or row.get('v6_independent_tuple')or row.get('resolved_tuple',row.get('historical_resolved_tuple'))
   if not value:continue
   v=list(map(int,value.split(',')));n=int(row.get('source_size',row.get('prefix_size')))
   need(classify(v,n)==row['v8'],'unchanged upstream admission inventory');count+=1
  inventories[path.name]=dict(rows=count,sha256=sha(path))
 manifest=dict(input_sha256=PINS,schema_version=1,upstream_revision=prior.REVISION,predecessor_cases=1433,case_count=len(cases),source_bytes=sum(len(c.source)for c in cases),classifications=dict(collections.Counter(c.expected for c in cases)),recipes=[c.metadata()for c in cases],resolved_tuples=tuples,upstream_admission=inventories,qualification='Source-only hypotheses. Every predecessor source/request/tuple/classification remains identical. New strategy7source16385..32768 is proposed only within window and geometry caps. No public enabling, native execution, or C/Rust entropy identity claimed.')
 return cases,manifest
if __name__=='__main__':
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--root',type=Path,default=ROOT);parser.add_argument('--write',type=Path);a=parser.parse_args()
 def guard(event,args):
  if event in ('subprocess.Popen','os.system','os.fork','os.posix_spawn'):raise ValueError('source-only children forbidden')
 sys.addaudithook(guard)
 cases,manifest=prepare(a.root)
 data=(json.dumps(manifest,sort_keys=True,indent=2)+'\n').encode()
 if a.write:
  a.write.parent.mkdir(parents=True,exist_ok=True)
  with a.write.open('xb')as f:f.write(data)
 print(json.dumps(dict(status='passed',cases=len(cases),classifications=manifest['classifications'],manifest_sha256=hashlib.sha256(data).hexdigest(),child_processes=0),sort_keys=True))
