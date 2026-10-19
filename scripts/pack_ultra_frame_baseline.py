#!/usr/bin/env python3
"""Project 26 preselected complete public frames after full offline native replay."""
from pathlib import Path
import argparse,hashlib,json,struct,sys
sys.dont_write_bytecode=True
ROOT=Path(__file__).resolve().parents[1]
NAMES=tuple([f'ultra-default-{level}-{size}-{pattern}'for level in (13,14)for size in (64,2048)for pattern in ('uniform','periodic','noise')]+[f'ultra-minimum-{minimum}-target-{target}'for minimum in range(3,8)for target in (1,4095)]+['ultra-observed-uf_p2_fresh_wide_horizon','ultra-observed-uf_p2_fresh_wide_window16','ultra-observed-uf_p2_alternative_seed_0','ultra-observed-uf_p2_alternative_seed_2'])
def need(v,s):
 if not v:raise ValueError(s)
def sha(b):return hashlib.sha256(b).hexdigest()
def fnv(data):
 v=0xcbf29ce484222325
 for b in data:v=((v^b)*0x100000001b3)&((1<<64)-1)
 return f'{v:016x}'
def project(capture,destination):
 import check_ultra_interop_capture as auditor
 import check_ultra_interop_recipes as inputs
 from oracles import ultra_interop as generator,greedy_interop_protocol as protocol
 totals=auditor.check(capture);manifest=inputs.check();cases=generator.recipes();evidence=(capture/'evidence.json').read_bytes()
 pack=bytearray(b'ZUBF0001'+struct.pack('<I',26));rows=[]
 for name in NAMES:
  index=next(i for i,c in enumerate(cases)if c.name==name);case=cases[index];need(case.expected=='success','preselected admitted source')
  status,resolved,frame=protocol.response((capture/'logs'/f'case-{index:03}-rust-encode-safe.stdout').read_bytes(),b'ZGOR0001')
  need(status==0 and resolved==manifest['resolved_tuples'][index],'exact successful independent tuple')
  need((status,resolved,frame)==protocol.response((capture/'logs'/f'case-{index:03}-rust-encode-all.stdout').read_bytes(),b'ZGOR0001'),'full backend identity')
  for mode in ('normal','sanitized'):
   status,zeros,decoded=protocol.response((capture/'logs'/f'case-{index:03}-c-{mode}-decode-safe.stdout').read_bytes(),b'ZGOC0001')
   need(status==0 and zeros==[0]*7 and decoded==case.source,'independent C decoded baseline frame')
  offset=len(pack);encoded=name.encode('ascii');pack+=struct.pack('<I',len(encoded))+encoded+struct.pack('<9q',*case.request)+struct.pack('<7I',*resolved)+struct.pack('<II',len(case.source),len(frame))+case.source+frame
  rows.append(dict(name=name,index=index,request=list(case.request),resolved=resolved,bytes=len(case.source),fnv1a64=fnv(case.source),sha256=sha(case.source),frame_bytes=len(frame),frame_fnv1a64=fnv(frame),frame_sha256=sha(frame),offset=offset))
 need(len(rows)==26 and len(pack)<=2<<20,'bounded complete pack')
 provenance=dict(schema_version=1,upstream_revision=generator.REVISION,recipe_manifest_sha256=sha(inputs.MANIFEST.read_bytes()),capture_evidence_sha256=sha(evidence),capture_totals=totals,pack_bytes=len(pack),pack_sha256=sha(pack),cases=rows,qualification='Preselected complete Rust public Ultra frames independently decoded by pinned normal and fully sanitized C; all safe/all frames equal. Projection follows complete offline native command/artifact replay. No C/Rust compressed-byte identity or timing claim.')
 encoded=(json.dumps(provenance,sort_keys=True,indent=2)+'\n').encode();destination.mkdir(parents=True,exist_ok=True)
 for name,data in [('cases.bin',bytes(pack)),('provenance.json',encoded)]:
  path=destination/name
  if path.exists():need(path.read_bytes()==data,'immutable existing projection')
  else:path.write_bytes(data)
 return dict(status='passed',cases=26,pack_bytes=len(pack),pack_sha256=sha(pack),provenance_sha256=sha(encoded))
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--capture',required=True,type=Path);p.add_argument('--output',type=Path,default=ROOT/'tests/fixtures/reference-ultra-frame-baseline');args=p.parse_args();print(json.dumps(project(args.capture.resolve(strict=True),args.output),sort_keys=True))
