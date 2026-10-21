#!/usr/bin/env python3
"""Check immutable native-audited complete retained Optimal baseline bytes without execution."""
from pathlib import Path
import argparse,hashlib,json,struct
ROOT=Path(__file__).resolve().parents[1]
COLLECTION='tests/fixtures/reference-retained-optimal-frame-baseline'
PACK_SHA='7804afd8512933ec0d94f09a0ccdc7b801580e484b99e07bdba0124672750ce0'
PROVENANCE_SHA='62687dd9c53c71328f7e1fabf2daf56dcc595d069664c6594ab225968ade8c2c'
MANIFEST_SHA='0fcd36c87dd5e1a2de6288c0b3e41abb2acd81b74060f37436baa07d46e57fde'
NAMES=('retained-wide_frames_uniform_16385', 'retained-wide_frames_uniform_24576', 'retained-wide_frames_uniform_24577', 'retained-wide_frames_uniform_32768', 'retained-wide_frames_copy', 'retained-wide_frames_mixed', 'retained-wide_frames_short', 'retained-wide_frames_minimum_5', 'retained-wide_frames_minimum_6', 'retained-wide_frames_minimum_7', 'retained-wide_frames_small_tables', 'retained-boundary-16385-noise', 'retained-boundary-16385-periodic', 'retained-boundary-24576-noise', 'retained-boundary-24576-periodic', 'retained-boundary-24577-noise', 'retained-boundary-24577-periodic', 'retained-boundary-32768-noise', 'retained-boundary-32768-periodic', 'retained-minimum-3-target-0-32768', 'retained-minimum-3-target-1-32768', 'retained-minimum-3-target-4095-32768', 'retained-geometry-15-6-6', 'retained-geometry-16-12-1', 'retained-minimum-4-target-1-16385', 'retained-minimum-7-target-0-32768')
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
 manifest=decode(checked_file(root,'tests/fixtures/reference-retained-optimal-public/recipes.json',1<<20,MANIFEST_SHA))
 need(set(provenance)=={'schema_version','upstream_revision','source_manifest_sha256','capture_evidence_sha256','native_archive_sha256','selection','pack_bytes','pack_sha256','cases','qualification'},'exact provenance schema')
 need(type(provenance['schema_version'])is int and provenance['schema_version']==1 and provenance['pack_bytes']==len(data) and provenance['pack_sha256']==PACK_SHA,'exact projection schema/extent')
 need(provenance['source_manifest_sha256']==MANIFEST_SHA and provenance['upstream_revision']=='f8745da6ff1ad1e7bab384bd1f9d742439278e99','pinned attribution/source recipe')
 need(provenance['native_archive_sha256']=='4653c33e7f536ec10271d7774a41cb34e3de7995b0d2c7cdf7b79ae7a99c90ae' and exact(provenance['selection'],list(NAMES)),'qualified native archive and preselection')
 need(type(provenance['capture_evidence_sha256'])is str and len(provenance['capture_evidence_sha256'])==64 and all(c in '0123456789abcdef'for c in provenance['capture_evidence_sha256']),'captured evidence binding')
 at=0
 def take(n):
  nonlocal at
  need(type(n)is int and 0<=n<=len(data)-at,'bounded complete baseline field')
  b=data[at:at+n];at+=n;return b
 def u32():return struct.unpack('<I',take(4))[0]
 need(take(8)==b'ZORB0001' and u32()==26,'exact pack schema/count')
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
