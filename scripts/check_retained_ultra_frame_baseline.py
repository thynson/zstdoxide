#!/usr/bin/env python3
"""Check immutable native-audited complete retained Ultra baseline bytes without execution."""
from pathlib import Path
import argparse,hashlib,json,struct
ROOT=Path(__file__).resolve().parents[1]
COLLECTION='tests/fixtures/reference-retained-ultra-frame-baseline'
PACK_SHA='9be9a2de5b1d4ea624185d45c7ebbfd2abbe18aa12af29c35175b1c525db82a2'
PROVENANCE_SHA='71f5b711a778ac082d44bd826d7e608d8bc9fb1ff4707ff1a6cc6a53822f3d36'
MANIFEST_SHA='510db8c9072d9f64f9919f8863b7b60bd1c2fd28e8c096172dc5dd1d2fb599db'
NAMES=('retained-ultra-ur_p2_wide_frames_uniform_16385', 'retained-ultra-ur_p2_wide_frames_uniform_24576', 'retained-ultra-ur_p2_wide_frames_uniform_24577', 'retained-ultra-ur_p2_wide_frames_uniform_32768', 'retained-ultra-ur_p2_wide_frames_copy', 'retained-ultra-ur_p2_wide_frames_mixed', 'retained-ultra-ur_p2_wide_frames_short', 'retained-ultra-ur_p2_wide_frames_minimum_5', 'retained-ultra-ur_p2_wide_frames_minimum_6', 'retained-ultra-ur_p2_wide_frames_minimum_7', 'retained-ultra-ur_p2_wide_frames_small_tables', 'retained-ultra-boundary-16385-noise', 'retained-ultra-boundary-16385-periodic', 'retained-ultra-boundary-24576-noise', 'retained-ultra-boundary-24576-periodic', 'retained-ultra-boundary-24577-noise', 'retained-ultra-boundary-24577-periodic', 'retained-ultra-boundary-32768-noise', 'retained-ultra-boundary-32768-periodic', 'retained-ultra-minimum-3-target-0-32768', 'retained-ultra-minimum-3-target-1-32768', 'retained-ultra-minimum-3-target-4095-32768', 'retained-ultra-geometry-15-6-6', 'retained-ultra-geometry-16-12-1', 'retained-ultra-minimum-4-target-1-16385', 'retained-ultra-minimum-7-target-0-32768')
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
 manifest=decode(checked_file(root,'tests/fixtures/reference-retained-ultra-public/recipes.json',1<<20,MANIFEST_SHA))
 need(set(provenance)=={'schema_version','upstream_revision','source_manifest_sha256','capture_evidence_sha256','native_archive_sha256','selection','pack_bytes','pack_sha256','cases','qualification'},'exact provenance schema')
 need(type(provenance['schema_version'])is int and provenance['schema_version']==1 and provenance['pack_bytes']==len(data) and provenance['pack_sha256']==PACK_SHA,'exact projection schema/extent')
 need(provenance['source_manifest_sha256']==MANIFEST_SHA and provenance['upstream_revision']=='f8745da6ff1ad1e7bab384bd1f9d742439278e99','pinned attribution/source recipe')
 need(provenance['native_archive_sha256']=='4bf5ba6509ea0096696994765e6d4c034d71b8480713374dcd1c7cfaadf332db' and exact(provenance['selection'],list(NAMES)),'qualified native archive and preselection')
 need(type(provenance['capture_evidence_sha256'])is str and len(provenance['capture_evidence_sha256'])==64 and all(c in '0123456789abcdef'for c in provenance['capture_evidence_sha256']),'captured evidence binding')
 at=0
 def take(n):
  nonlocal at
  need(type(n)is int and 0<=n<=len(data)-at,'bounded complete baseline field')
  b=data[at:at+n];at+=n;return b
 def u32():return struct.unpack('<I',take(4))[0]
 need(take(8)==b'ZURB0001' and u32()==26,'exact pack schema/count')
 need(type(provenance['cases'])is list and len(provenance['cases'])==26,'complete case metadata')
 for expected,record in zip(NAMES,provenance['cases'],strict=True):
  offset=at;length=u32();need(1<=length<=128,'bounded name');name=take(length).decode('ascii');need(name==expected,'preselected case order')
  request=list(struct.unpack('<9q',take(72)));resolved=list(struct.unpack('<7I',take(28)));n,m=u32(),u32();need(16385<=n<=32768 and 0<m<=n+7+3*((n+8191)//8192),'complete source/frame bounds')
  source,frame=take(n),take(m)
  index=next(i for i,c in enumerate(manifest['recipes'])if c['id']==name);recipe=manifest['recipes'][index]
  need(recipe['expected']=='success' and request==recipe['request'] and resolved==manifest['resolved_tuples'][index] and n==recipe['source_bytes'] and sha(source)==recipe['source_sha256'],'independent source-only request and tuple identity')
  from oracles.greedy_interop_protocol import frame_info
  info=frame_info(frame,n)
  need(len(info['blocks'])==(n+8191)//8192,'balanced retained block count')
  expected_record=dict(name=name,index=index,request=request,resolved=resolved,source_bytes=n,source_sha256=sha(source),frame_bytes=m,frame_sha256=sha(frame),block_kinds=[b['kind']for b in info['blocks']],offset=offset)
  need(type(record)is dict and exact(record,expected_record) and all(type(record[k])is int for k in ('index','source_bytes','frame_bytes','offset')),'complete case, byte and frame metadata')
  need(frame[:4]==b'\x28\xb5\x2f\xfd','standard complete frame magic')
 need(at==len(data),'no trailing baseline bytes')
 result=dict(provenance)
 result['cases']=[dict(r,bytes=r['source_bytes'],sha256=r['source_sha256'])for r in provenance['cases']]
 # Fingerprints supplement SHA-256 and exact pinned bytes; they are not the oracle.
 for r in result['cases']:
  start=r['offset']+4+len(r['name'])+72+28+8
  r['fnv1a64']=fnv(data[start:start+r['bytes']])
  r['frame_fnv1a64']=fnv(data[start+r['bytes']:start+r['bytes']+r['frame_bytes']])
 return result
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--root',type=Path,default=ROOT);a=p.parse_args();v=check(a.root);print(json.dumps(dict(status='passed',cases=len(v['cases']),pack_sha256=v['pack_sha256']),sort_keys=True))
