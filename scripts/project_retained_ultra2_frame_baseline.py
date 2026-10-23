"""Project preselected complete retained frames from independently decoded captures."""
from pathlib import Path
import argparse,hashlib,json,struct,sys
ROOT=Path('/Users/lanxingcan/Workspace/zstdoxize');CAPTURE=Path('/private/tmp/retained-ultra2-public-capture')
NAMES=tuple(['retained-ultra2-u2r_wide_frames_uniform_'+str(n)for n in [16385,24576,24577,32768]]+['retained-ultra2-u2r_wide_frames_'+n for n in ['copy','mixed','short','minimum_5','minimum_6','minimum_7','small_tables']]+[f'retained-ultra2-boundary-{n}-{kind}'for n in [16385,24576,24577,32768]for kind in ['noise','periodic']]+[f'retained-ultra2-minimum-3-target-{t}-32768'for t in [0,1,4095]]+['retained-ultra2-geometry-15-6-6','retained-ultra2-geometry-16-12-1','retained-ultra2-minimum-4-target-1-16385','retained-ultra2-minimum-7-target-0-32768'])
def need(ok,why):
 if not ok:raise ValueError(why)
def sha(b):return hashlib.sha256(b).hexdigest()
def project(output):
 need(len(NAMES)==len(set(NAMES))==26,'preselected unique source/parameter coverage')
 sys.path.insert(0,str(ROOT/'scripts'));import plan_retained_ultra2_public as hypotheses
 from oracles import greedy_interop_protocol as protocol
 cases,manifest=hypotheses.prepare(ROOT);need(sha((ROOT/'tests/fixtures/reference-retained-ultra2-public/recipes.json').read_bytes())=='8208d9871ce2ed9d508b704178b7df8ed5c9d29665c70c6765218d0d2548afaf','immutable source manifest')
 evidence=json.loads((CAPTURE/'evidence.json').read_bytes());need(evidence['status']=='passed'and evidence['source_unchanged'],'completed native capture')
 commands={c['label']:c for c in evidence['commands']};metadata={c.name:(i,c)for i,c in enumerate(cases)}
 def response(label,request,magic):
  c=commands[label];need(c['exit_code']==c['expected_exit']==0,'completed exact command')
  packet=(CAPTURE/c['stdin']).read_bytes();need(packet==request and sha(packet)==c['stdin_sha256'],'independent request bytes')
  b=(CAPTURE/c['stdout']['path']).read_bytes();need(len(b)==c['stdout']['bytes']and sha(b)==c['stdout']['sha256'],'exact recorded response');return protocol.response(b,magic)
 pack=bytearray(b'ZU2B0001'+struct.pack('<I',26));rows=[]
 for name in NAMES:
  i,c=metadata[name];v=manifest['resolved_tuples'][i];need(c.expected=='success'and v[6]==9 and 16385<=len(c.source)<=32768,'retained public case')
  frames=[]
  for mode in ['safe','all']:
   status,resolved,frame=response(f'case-{i}-rust-encode-{mode}',protocol.packet(c.source,c.request),b'ZGOR0001');need(status==0 and resolved==v,'qualified Rust tuple');frames.append(frame)
   for native in ['normal','sanitized']:
    status,zeros,decoded=response(f'case-{i}-c-{native}-decode-{mode}',protocol.packet(c.source,c.request,frame,1),b'ZGOC0001');need(status==0 and zeros==[0]*7 and decoded==c.source,'complete native decoded source')
  need(frames[0]==frames[1],'safe/all complete frame bytes');frame=frames[0];info=protocol.frame_info(frame,len(c.source));need(len(info['blocks'])==(len(c.source)+8191)//8192,'balanced schedule')
  offset=len(pack);encoded=name.encode('ascii');pack+=struct.pack('<I',len(encoded))+encoded+struct.pack('<9q',*c.request)+struct.pack('<7I',*v)+struct.pack('<2I',len(c.source),len(frame))+c.source+frame
  rows.append(dict(name=name,index=i,request=c.request,resolved=v,source_bytes=len(c.source),source_sha256=sha(c.source),frame_bytes=len(frame),frame_sha256=sha(frame),block_kinds=[b['kind']for b in info['blocks']],offset=offset))
 need(len(pack)<=2<<20,'bounded complete-frame pack')
 provenance=dict(schema_version=1,upstream_revision=manifest['upstream_revision'],selection=NAMES,source_manifest_sha256=sha((ROOT/'tests/fixtures/reference-retained-ultra2-public/recipes.json').read_bytes()),capture_evidence_sha256=sha((CAPTURE/'evidence.json').read_bytes()),native_archive_sha256='a95b599c9b68f4a2013ba61d2bbbe8dad273e4911b9ad1f9850967bc0b94621f',pack_sha256=sha(pack),pack_bytes=len(pack),cases=rows,qualification='Twenty-six complete retained source/request/frame cases selected before clocks for four source boundaries, raw/RLE/compressed and cross-block histories, every minimum, target requests and table/search endpoints. Every safe/all frame and its normal/sanitized native decoded source are byte-checked. No allocator ledger or timing is inferred; those are subsequent measurements.')
 if output:
  output.mkdir();(output/'cases.bin').write_bytes(pack);(output/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n');(output/'capture-evidence.json').write_bytes((CAPTURE/'evidence.json').read_bytes())
 return dict(status='passed',cases=26,pack_bytes=len(pack),pack_sha256=sha(pack),source_bytes=sum(r['source_bytes']for r in rows),frame_bytes=sum(r['frame_bytes']for r in rows),child_processes=0)
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--write',type=Path);a=p.parse_args()
 def guard(event,args):
  if event in ['subprocess.Popen','os.system','os.fork','os.posix_spawn']:raise ValueError('projection forbids children')
 sys.addaudithook(guard);print(json.dumps(project(a.write),sort_keys=True))
