#!/usr/bin/env python3
"""Check immutable native-audited complete Ultra2 baseline bytes without execution."""
from pathlib import Path
import argparse,hashlib,json,struct
ROOT=Path(__file__).resolve().parents[1]
COLLECTION='tests/fixtures/reference-ultra2-frame-baseline'
PACK_SHA='5800d0e13d324f2401f2739e1482ad8d9eb8afe42f2118933afa42b7b1b3896f'
PROVENANCE_SHA='2b234e63470bcba505dc6ed459537011613e37635aa7205610597afbf357e1b3'
MANIFEST_SHA='0ae6c7cfb5ab3530c52976139326514e9919bcba321d9e6516621ffd129f4dd5'
NAMES=tuple([f'ultra2-default-{level}-{size}-{pattern}'for level in (16,17)for size in (64,2048)for pattern in ('uniform','periodic','noise')]+[f'ultra2-minimum-{minimum}-target-{target}'for minimum in range(3,8)for target in (1,4095)]+['ultra2-observed-u2_seed_fresh_wide_horizon','ultra2-observed-u2_seed_fresh_wide_window16','ultra2-observed-u2_seed_alternative_seed_0','ultra2-observed-u2_seed_alternative_seed_2'])
def need(v,s):
 if not v:raise ValueError(s)
def sha(b):return hashlib.sha256(b).hexdigest()
def exact(a,b):
 if type(a)is not type(b):return False
 if type(a)is dict:return a.keys()==b.keys() and all(exact(a[k],b[k])for k in a)
 if type(a)is list:return len(a)==len(b) and all(exact(x,y)for x,y in zip(a,b))
 return a==b
def fnv(data):
 v=0xcbf29ce484222325
 for byte in data:v=((v^byte)*0x100000001b3)&0xffffffffffffffff
 return f'{v:016x}'
def decode(data):
 def pairs(values):
  result={}
  for k,v in values:
   need(k not in result,'duplicate JSON field');result[k]=v
  return result
 def invalid(value):raise ValueError('nonfinite JSON')
 return json.loads(data,object_pairs_hook=pairs,parse_constant=invalid)
def checked_file(root,name,maximum,digest):
 p=root/name;need(p.is_file() and not p.is_symlink() and p.resolve().is_relative_to(root.resolve()) and p.stat().st_size<=maximum,'bounded regular baseline file')
 b=p.read_bytes();need(sha(b)==digest,'immutable baseline digest '+name);return b
def check(root=ROOT):
 root=Path(root)
 data=checked_file(root,COLLECTION+'/cases.bin',2<<20,PACK_SHA)
 provenance=decode(checked_file(root,COLLECTION+'/provenance.json',1<<20,PROVENANCE_SHA))
 manifest=decode(checked_file(root,'tests/fixtures/reference-ultra2-interop-recipes/recipes.json',1<<20,MANIFEST_SHA))
 need(set(provenance)=={'schema_version','upstream_revision','recipe_manifest_sha256','capture_evidence_sha256','capture_totals','pack_bytes','pack_sha256','cases','qualification'},'exact provenance schema')
 need(type(provenance['schema_version'])is int and provenance['schema_version']==1 and provenance['pack_bytes']==len(data) and provenance['pack_sha256']==PACK_SHA,'exact projection schema/extent')
 need(provenance['recipe_manifest_sha256']==MANIFEST_SHA and provenance['upstream_revision']=='f8745da6ff1ad1e7bab384bd1f9d742439278e99','pinned attribution/source recipe')
 need(exact(provenance['capture_totals'],dict(recipes=1433,admitted=1292,unsupported=0,resource=141,rust_frames_c_decoded=5168,c_frames_rust_decoded=5168,decoded_bytes=802569512)),'complete bidirectional capture qualification')
 need(type(provenance['capture_evidence_sha256'])is str and len(provenance['capture_evidence_sha256'])==64 and all(c in '0123456789abcdef'for c in provenance['capture_evidence_sha256']),'captured evidence binding')
 at=0
 def take(n):
  nonlocal at
  need(type(n)is int and 0<=n<=len(data)-at,'bounded complete baseline field')
  b=data[at:at+n];at+=n;return b
 def u32():return struct.unpack('<I',take(4))[0]
 need(take(8)==b'ZU2B0001' and u32()==26,'exact pack schema/count')
 need(type(provenance['cases'])is list and len(provenance['cases'])==26,'complete case metadata')
 for expected,record in zip(NAMES,provenance['cases'],strict=True):
  offset=at;length=u32();need(1<=length<=128,'bounded name');name=take(length).decode('ascii');need(name==expected,'preselected case order')
  request=list(struct.unpack('<9q',take(72)));resolved=list(struct.unpack('<7I',take(28)));n,m=u32(),u32();need(64<=n<=16384 and 0<m<=n+32,'complete source/frame bounds')
  source,frame=take(n),take(m)
  index=next(i for i,c in enumerate(manifest['recipes'])if c['id']==name);recipe=manifest['recipes'][index]
  need(recipe['expected']=='success' and request==recipe['request'] and resolved==manifest['resolved_tuples'][index] and n==recipe['source_bytes'] and sha(source)==recipe['source_sha256'],'independent source-only request and tuple identity')
  expected_record=dict(name=name,index=index,request=request,resolved=resolved,bytes=n,fnv1a64=fnv(source),sha256=sha(source),frame_bytes=m,frame_fnv1a64=fnv(frame),frame_sha256=sha(frame),offset=offset)
  need(type(record)is dict and exact(record,expected_record) and all(type(record[k])is int for k in ('index','bytes','frame_bytes','offset')),'complete case, byte and frame metadata')
  need(frame[:4]==b'\x28\xb5\x2f\xfd','standard complete frame magic')
 need(at==len(data),'no trailing baseline bytes')
 return provenance
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--root',type=Path,default=ROOT);a=p.parse_args();v=check(a.root);print(json.dumps(dict(status='passed',cases=len(v['cases']),pack_sha256=v['pack_sha256']),sort_keys=True))
