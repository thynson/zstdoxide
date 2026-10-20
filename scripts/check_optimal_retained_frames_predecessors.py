#!/usr/bin/env python3
"""Reconcile all prior native frames, ledgers and observations after new capture."""
import argparse,hashlib,json,sys,tarfile
from pathlib import Path
from oracles.optimal_retained_frames import require
from oracles.optimal_retained_wide_recipes import recipes
R=Path(__file__).resolve().parents[1]
def sha(data):return hashlib.sha256(data).hexdigest()
def check(root):
 children=[]
 def guard(event,args):
  if event in ('subprocess.Popen','os.system','os.fork','os.posix_spawn'):
   children.append(event);raise ValueError('no child execution')
 sys.addaudithook(guard)
 e=json.loads((root/'evidence.json').read_text());require(e['status']=='passed' and e['audit']['recipes']==19,'completed larger capture')
 report=json.loads((R/'docs/evidence/2026-10-16-optimal-retained-wide-capture.json').read_text());path=R/'docs/evidence'/report['archive']
 require(path.stat().st_size==report['archive_bytes'] and sha(path.read_bytes())==report['archive_sha256'],'immutable predecessor archive')
 expected={v['path']:v for v in report['files']};seen=set();frames=ledgers=transcripts=packets=0
 with tarfile.open(path,'r|gz')as tar:
  for member in tar:
   require(member.isfile() and member.name in expected and member.name not in seen,'exact predecessor inventory');seen.add(member.name)
   data=tar.extractfile(member).read();v=expected[member.name];require(len(data)==v['bytes'] and sha(data)==v['sha256'],'exact predecessor member')
   name=member.name
   if name.startswith('artifacts/'):
    require((root/name).read_bytes()==data,'unchanged complete predecessor frame or ledger')
    frames+=int(name.endswith('.zst'));ledgers+=int(name.endswith('.json'))
   elif name.startswith('logs/') and '-run-wide_'in name and name.endswith('.stdout'):
    header,rest=data.split(b'\n',1);require(b'"protocol":"ZOPRW02"'in header,'original metadata identity')
    expected_data=header.replace(b'"protocol":"ZOPRW02"',b'"protocol":"ZOPRS03"')+b'\n'+rest
    require((root/name).read_bytes()==expected_data,'unchanged complete predecessor states except protocol metadata');transcripts+=1
   elif name.startswith('requests/wide_'):
    require((root/name).read_bytes()[8:]==data[8:],'unchanged predecessor packet body');packets+=1
 require(seen==set(expected) and (frames,ledgers,transcripts,packets)==(28,28,14,7),'complete predecessor reconciliation')
 require(not children,'no children')
 return dict(status='passed',recipes=len(recipes()),frames=frames,ledgers=ledgers,transcripts=transcripts,packets=packets,child_processes=0,predecessor_archive_sha256=report['archive_sha256'],qualification='All old normal/sanitized and padding frames/ledgers are byte-identical; both full transcripts differ only in protocol metadata. Packet bodies are unchanged.')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('capture',type=Path);print(json.dumps(check(p.parse_args().capture),sort_keys=True))
